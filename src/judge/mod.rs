extern crate libc;

use std::{ ffi::CString, fs, io::Write };
use std::{ path::Path, process::Command };
use std::{ sync::mpsc, thread };

use super::executor;

mod debugger;

pub fn setup(dir: String) -> (thread::JoinHandle<()>, mpsc::Sender<ToMark>, mpsc::Receiver<ToSend>) {
    let (s_ptj, r_ptj) = mpsc::channel();
    let (s_jtp, r_jtp) = mpsc::channel();
    (thread::spawn(move || run(dir, s_jtp, r_ptj)), s_ptj, r_jtp)
}

pub struct ToMark {
    pub batch:      u32,
    pub answer:     String,
    pub lang:       String,
    pub max_time:   Option<u64>,
    pub case_in:    Vec<String>,
    pub case_out:   Vec<String>
}

#[derive(Clone, Copy)]
pub struct ToSend {
    pub batch:      u32,
    pub case:       u64,
    pub result:     MarkResult
}

#[derive(Clone, Copy)]
pub enum MarkResult {
    Fail,
    Success(i64, i64),
    CE,
    RTE,
    TLE,
    Blocked(u32)
}

fn run(dir: String, sender: mpsc::Sender<ToSend>, recver: mpsc::Receiver<ToMark>) {
    let exec_dir = Path::new(&dir);
    for input in recver.iter() {
        // Pre-run compilation/preparing
        let mut lang = input.lang;
        lang.push_str(".yaml");
        let mut exec = fs::File::open(exec_dir.join(Path::new(&lang))).unwrap();
        let executor = executor::Executor::from_file(&mut exec);
        let mut sub = fs::File::open(executor.filename).unwrap();
        write!(sub, "{}", input.answer).unwrap();
        sub.flush().unwrap();
        if let Some(pre_exec) = executor.pre_exec {
            let vec_args: Vec<&str> = pre_exec.split_whitespace().collect();
            match Command::new(vec_args[0]).args(vec_args[1..].iter()).spawn().unwrap().wait() {
                Ok(exit) => if !exit.success() {
                    sender.send(ToSend {
                        batch:      input.batch,
                        case:       0,
                        result:     MarkResult::CE
                    }).unwrap();
                    continue;
                },
                Err(_) => {
                    sender.send(ToSend {
                        batch:      input.batch,
                        case:       0,
                        result:     MarkResult::CE
                    }).unwrap();
                    continue;
                }
            }
        }
        for case_num in 1..(input.case_in.len()+1) {
            let vec_args: Vec<&str> = executor.exec.split_whitespace().collect();
            let mut process = debugger::Process::new(vec_args[0], &vec_args[1..], input.max_time);
            process.run();
            let stdin = CString::new(input.case_in[case_num-1].clone()).unwrap();
            let i = stdin.as_bytes_with_nul();
            unsafe {
                libc::write(process.stdin, i.as_ptr() as _, i.len());
            }
            { // Give debugger an explicit lifetime
                let mut debugger = debugger::Debugger::standard(&mut process);
                debugger.monitor();
            }
            let mut output = String::new();
            let mut b8 = 0u8;
            while unsafe { libc::read(process.stdout, &mut b8 as *mut u8 as _, 1) == 1 } {
                output.push(b8 as char);
            }
            let result = match process.reason {
                MarkResult::Success(s, ns) => { 
                    match input.max_time {
                        Some(x) => {
                            if s as f64 + (ns as f64 / 1e6) > x as f64 {
                                MarkResult::TLE
                            } else {
                                if output == input.case_out[case_num-1] {
                                    MarkResult::Success(s, ns)
                                } else {
                                    MarkResult::Fail
                                }
                            }
                        }
                        None => {
                            if output == input.case_out[case_num-1] {
                                MarkResult::Success(s, ns)
                            } else {
                                MarkResult::Fail
                            }
                        }
                    }
                },
                x => x
            };
            sender.send(ToSend {
                batch:      input.batch,
                case:       case_num as u64,
                result:     result
            }).unwrap();
        }
    }
}
