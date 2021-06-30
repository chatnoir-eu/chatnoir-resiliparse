# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir](https://github.com/chatnoir-eu/) web data processing pipeline.


## ProcessGuard
The Resiliparse ProcessGuard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits for execution time and memory usage. ProcessGuard helps to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.
