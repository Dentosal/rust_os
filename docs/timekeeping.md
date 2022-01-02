# Timekeeping

Several date-and-time related functionalities are needed by an OS and applications:
* Scheduler timeslices: for sleeping and pre-emption
* Showing current date and time to the user
* Network applications, e.g. TLS certificate expiration
* Filesystem timestamps, including cremovable media as well
* Showing current date and time to the user
* Applications: timeouts, delays and such

## Theory of clocks

Clocks can have some of the following properties:
* monotonic: the value never decreases
* steady: increases linearly with physical time, monotonic
* system-wide: value is same for all reading programs
* wall-clock-time: convertible to the real world time

Providing all of the properties in a single clock value is not practical:
* wall-clock-time needs to be adjusted from an external source
* system-wide clock is too expensive to use for interval-measuring

## Available clocks

* Process-local steady clock
* System-wide almost-monotonic clock
* Wall-clock time (both monotonic and immediately ajdusting variants)

### Process-local steady clock

The time is only usable within a single process, but is extremely cheap to read; it uses `TSC` so no system calls are required to get a value.

An per-CPU configuration is stored into a shared memory region. It contains frequency of the `TSC` in Hz, as well as offset value which can be used to compare `TSC` values across CPUs.

### System-wide almost-monotonic clock

This is a the process-local clock, but the exact monotocity-guarantee is lost when using it over process boundaries. However, it shouldn't go back more than a microseconds, and should generally be usable for inter-process comminication and system-wide timestamps.

### Wall-clock time

Wall-clock time is converible to read world date-and-time values. There are both monotonic and immediately-adjusting variants available. The monotonic variant speeds up if this is before the immediately-adjusting variant, and slows down if it's before.

The immediately-adjusting variant is the best available real-world time available to the system. If a network connection is available, it will be used to get the current timestamp, probably using `NTP`, `TIME` or even `HTTP`. If no network connection is available, the battery-backed `RTC` will be used.

The wall-clock time is not available in the operating system. Instead, it's provided by the time service, which manages `RTC` and network time.

## Real-world timekeeping considerations

### Time zones and daylight savings

The system time is always stored in UTC. A global setting in the time service is used to store current timezone and possible daylight savings settings, and conversion functions to local time are available.

### Dates, including leap years

Timestamp-to-data conversions are available as functions.
