use super::MarkResult;

extern crate libc;

use std::collections::BTreeSet;
use std::ffi::*;

pub struct Process {
    exe:        CString,
    args:       Vec<CString>,
    pid:        Option<libc::pid_t>,
    pub stdin:  i32,
    pub stdout: i32,
    pub reason: MarkResult,
    pub m_time: Option<libc::rlim_t>
}
impl Process {
    pub fn new<S: Into<Vec<u8>> + Clone>(file: S, args: &[S], max_time: Option<libc::rlim_t>) -> Process {
        use std::ptr;
        let mut v = Vec::new();
        for i in args {
            let a = i.clone();
            v.push(CString::new(a).unwrap());
        }
        v.push(unsafe { CString::from_raw(ptr::null_mut()) });
        Process {
            exe:    CString::new(file).unwrap(),
            args:   v,
            pid:    None,
            stdin:  0,
            stdout: 0,
            reason: MarkResult::Fail,
            m_time: max_time
        }
    }
    fn prepare_child(ptc: [i32;2], ctp: [i32;2]) {
        unsafe {
            libc::close(ptc[1]);
            libc::dup2(ptc[0], 0);
            libc::close(ctp[0]);
            libc::dup2(ptc[1], 1);
        }
    }
    fn prepare_parent(ctp: [i32;2], ptc: [i32;2]) {
        unsafe {
            libc::close(ctp[1]);
            libc::close(ptc[0]);
        }
    }
    pub fn run(&mut self) {
        use std::process;
        let mut ptc = [0i32;2];
        let mut ctp = [0i32;2];
        unsafe { 
            libc::pipe(&mut ptc as *mut [i32] as _);
            libc::pipe(&mut ctp as *mut [i32] as _);
        }
        let pid: libc::pid_t = unsafe { libc::fork() };
        if pid == 0 {
            use std::ptr;
            Self::prepare_child(ptc, ctp);

            unsafe { libc::setpgid(0, 0) };

            let mut v = Vec::new();
            for s in self.args.clone() {
                v.push(s.as_ptr());
            }

            if let Some(time) = self.m_time {
                unsafe {
                    use std::mem;
                    let mut pass: libc::rlimit = mem::zeroed();
                    pass.rlim_cur = time;
                    pass.rlim_max = time+1;
                    libc::setrlimit(libc::RLIMIT_CPU, &pass);
                }
            }

            unsafe {
                libc::ptrace(libc::PTRACE_TRACEME, 0, ptr::null_mut::<libc::c_void>(), ptr::null_mut::<libc::c_void>());
                libc::kill(libc::getpid(), libc::SIGSTOP);
                libc::execv(self.exe.as_ptr(), &v[0]);
                process::exit(1);
            }
        } else if pid == -1 {
            process::exit(1); // Temporary
        } else {
            Self::prepare_parent(ctp, ptc);
            self.pid = Some(pid);
            self.stdin = ptc[1];
            self.stdout = ctp[0];
        }
    }
}

pub struct Debugger<'a> {
    process:    &'a mut Process,
    handlers:   BTreeSet<u64>,
    children:   BTreeSet<i32>
}
impl<'a> Debugger<'a> {
    pub fn new(process: &'a mut Process) -> Debugger {
        Debugger {
            process:    process,
            handlers:   BTreeSet::new(),
            children:   BTreeSet::new()
        }
    }
    pub fn add_handler(&mut self, handlers: &[u64]) {
        for syscall in handlers {
            self.handlers.insert(*syscall);
        }
    }
    pub fn standard(process: &'a mut Process) -> Debugger {
        let mut d = Debugger::new(process);
        d.add_handler(&[
                      0,   // read
                      1,   // write
                      3,   // close
                      5,   // fstat
                      9,   // mmap
                      10,  // mmap
                      11,  // munmap
                      12,  // brk
                      21,  // access
                      158, // arch_prctl
                      231, // exit_group
                      257, // openat
        ]);
        d
    }
    fn kill_children(&mut self) {
        let mut v = Vec::new();
        for child in self.children.iter() {
            unsafe { libc::kill(*child, libc::SIGKILL) };
            v.push(*child);
        }
        for child in v {
            self.children.remove(&child);
        }
        return;
    }
    pub fn monitor(&mut self) {
        use std::{ mem, thread, time };
        thread::sleep(time::Duration::from_millis(100));

        let mut ru: libc::rusage = unsafe { mem::zeroed() };
        let mut first = true;
        let mut entering = true;
        let mut spawned = false;
        let mut status = 0;
        let mut signal = 0;
        let mut pid = if let Some(p) = self.process.pid {
            p
        } else { return; };
        let p_pid = pid;

        loop {
            unsafe { 
                pid = libc::wait4(-p_pid, &mut status, libc::__WALL, &mut ru);

                let mut syscall = false;

                if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) || status == 0 {
                    if pid == p_pid || first {
                        self.kill_children();
                        if let MarkResult::Blocked(_) = self.process.reason { return; }
                        else {
                            if status != 0 {
                                if status == libc::SIGKILL || status == libc::SIGXCPU {
                                    self.process.reason = MarkResult::TLE;
                                    return;
                                }
                                self.process.reason = MarkResult::RTE;
                                return;
                            }
                            break;
                        }
                    }
                    self.children.remove(&pid);
                }
                if first {
                    libc::ptrace(libc::PTRACE_SETOPTIONS, pid, 0, libc::PTRACE_O_TRACESYSGOOD
                                 | libc::PTRACE_O_TRACEEXIT | libc::PTRACE_O_EXITKILL | libc::PTRACE_O_TRACECLONE
                                 | libc::PTRACE_O_TRACEFORK | libc::PTRACE_O_TRACEVFORK);
                }
                if libc::WSTOPSIG(status) == (0x80 | libc::SIGTRAP) {
                    syscall = true;
                    let mut regs: libc::user_regs_struct = mem::zeroed();
                    libc::ptrace(libc::PTRACE_GETREGS, pid, 0, &mut regs);
                    if regs.orig_rax == libc::SYS_execve as u64 && !spawned {
                        if !entering {
                            spawned = true;
                        }
                        entering = !entering;
                        libc::ptrace(libc::PTRACE_SYSCALL, pid, 0, 0);
                        continue;
                    }
                    if entering && !self.handlers.contains(&regs.orig_rax) {
                        // KILL IT WITH FIRE
                        self.process.reason = MarkResult::Blocked(regs.orig_rax as u32);
                        libc::kill(p_pid, libc::SIGKILL);
                    }
                    entering = !entering;
                } else {
                    match libc::WSTOPSIG(status) {
                        libc::SIGTRAP => {
                            match status >> 16 {
                                libc::PTRACE_EVENT_FORK | libc::PTRACE_EVENT_VFORK => {
                                    let mut pid = 0u64;
                                    libc::ptrace(libc::PTRACE_GETEVENTMSG, pid, 0, &mut pid as *mut u64 as usize);
                                    self.children.insert(pid as i32);
                                },
                                _ =>  {}
                            }
                        },
                        x => signal = x
                    }
                }
                libc::ptrace(if syscall { libc::PTRACE_SYSCALL } else { libc::PTRACE_CONT }, pid, 0, if first { 0 } else { signal });
                first = false;
            }
        }
        self.process.reason = MarkResult::Success(ru.ru_utime.tv_sec, ru.ru_utime.tv_usec);
    }
}
