---------------------------- MODULE Scheduler ----------------------------
EXTENDS Naturals

CONSTANTS
  MaxRetries = 3
  MaxPriority = 10

VARIABLES
  actors,
  ready,
  running,
  paused,
  stopped,
  failed,
  retries,
  priorities,
  nextActor

TypeInvariant ==
  /\ ready \subseteq actors
  /\ running \subseteq actors
  /\ paused \subseteq actors
  /\ stopped \subseteq actors
  /\ failed \subseteq actors
  /\ DOMAIN retries = actors
  /\ DOMAIN priorities = actors
  /\ Cardinality(running) <= 1

StatePartition ==
  /\ actors = ready \cup running \cup paused \cup stopped \cup failed

Init ==
  /\ actors = {}
  /\ ready = {}
  /\ running = {}
  /\ paused = {}
  /\ stopped = {}
  /\ failed = {}
  /\ retries = {}
  /\ priorities = {}

Start(a) ==
  /\ a \notin actors
  /\ actors' = actors \cup {a}
  /\ ready' = ready \cup {a}
  /\ running' = running
  /\ paused' = paused
  /\ stopped' = stopped
  /\ failed' = failed
  /\ retries' = [retries EXCEPT ![a] = 0]
  /\ priorities' = [priorities EXCEPT ![a] = 0]

Pause(a) ==
  /\ a \in running
  /\ running' = running \ {a}
  /\ paused' = paused \cup {a}
  /\ ready' = ready
  /\ actors' = actors
  /\ stopped' = stopped
  /\ failed' = failed
  /\ retries' = retries
  /\ priorities' = priorities

Resume(a) ==
  /\ a \in paused
  /\ paused' = paused \ {a}
  /\ ready' = ready \cup {a}
  /\ running' = running
  /\ actors' = actors
  /\ stopped' = stopped
  /\ failed' = failed
  /\ retries' = retries
  /\ priorities' = priorities

Stop(a) ==
  /\ a \in (ready \cup running \cup paused)
  /\ ready' = ready \ {a}
  /\ running' = running \ {a}
  /\ paused' = paused \ {a}
  /\ stopped' = stopped \cup {a}
  /\ actors' = actors
  /\ failed' = failed
  /\ retries' = retries
  /\ priorities' = priorities

Fail(a) ==
  /\ a \in running
  /\ running' = running \ {a}
  /\ retries[a] < MaxRetries
  /\ retries' = [retries EXCEPT ![a] = retries[a] + 1]
  /\ ready' = ready \cup {a}
  /\ actors' = actors
  /\ paused' = paused
  /\ stopped' = stopped
  /\ failed' = failed
  /\ priorities' = priorities

FailPermanent(a) ==
  /\ a \in running
  /\ running' = running \ {a}
  /\ retries[a] = MaxRetries
  /\ failed' = failed \cup {a}
  /\ ready' = ready
  /\ actors' = actors
  /\ paused' = paused
  /\ stopped' = stopped
  /\ retries' = retries
  /\ priorities' = priorities

Schedule ==
  /\ Cardinality(running) = 0
  /\ ready # {}
  /\ LET next == CHOOSE a \in ready :
        /\ priorities[a] = Max({priorities[b] : b \in ready})
    IN
    /\ nextActor' = next
    /\ running' = {next}
    /\ ready' = ready \ {next}
    /\ actors' = actors
    /\ paused' = paused
    /\ stopped' = stopped
    /\ failed' = failed
    /\ retries' = retries
    /\ priorities' = priorities

Stutter ==
  /\ UNCHANGED <<actors, ready, running, paused, stopped, failed,
                  retries, priorities, nextActor>>

Next ==
  \/ \E a : Start(a)
  \/ \E a : Pause(a)
  \/ \E a : Resume(a)
  \/ \E a : Stop(a)
  \/ \E a : Fail(a)
  \/ \E a : FailPermanent(a)
  \/ Schedule
  \/ Stutter

Spec == Init /\ [][Next]_<<actors, ready, running, paused, stopped, failed,
                         retries, priorities, nextActor>>

NoConcurrentRun ==
  /\ Cardinality(running) <= 1

NoTaskLoss ==
  /\ \A a \in actors :
      ~(a \in ready \cup running \cup paused) =>
      a \in stopped \cup failed

PriorityOrdering ==
  /\ Cardinality(running) = 0 \/ Cardinality(running) = 1
  /\ Cardinality(running) = 1 =>
    \A a \in ready :
      priorities[nextActor] >= priorities[a]

THEOREM Spec => []TypeInvariant
THEOREM Spec => []StatePartition
THEOREM Spec => []NoConcurrentRun
THEOREM Spec => []PriorityOrdering

===============================================================================
\* TLC Model Checker Configuration
\*
\*   INIT Init
\*   NEXT Next
\*   INVARIANTS TypeInvariant, StatePartition, NoConcurrentRun, NoTaskLoss,
\*               PriorityOrdering
===============================================================================
