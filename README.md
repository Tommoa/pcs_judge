# pcs_judge - A judge for running programming competitions

This is the judge component of PCS. Here we handle running user code in as safe a manner as we can.

Implemented:
 - Linux sandboxer
 - Asynchronous communication to/from the server
 - Basic multiple language support
 
Roadmap:
 - Window sandboxer (not a high priority)
 - Better handling of the protocol between the judge and the server
 - Better "safe" async writing to the server
