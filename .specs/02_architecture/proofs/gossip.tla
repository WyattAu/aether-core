---------------------------- MODULE Gossip ----------------------------
EXTENDS Naturals

CONSTANTS
  PingInterval = 1000
  PingTimeout = 5000
  SuspectTimeout = 10000
  DeadTimeout = 30000

VARIABLES
  members,
  alive,
  suspect,
  dead,
  pingTargets,
  lastPing,
  incarnation

TypeInvariant ==
  /\ members = alive \cup suspect \cup dead

Init ==
  /\ members = {"self"}
  /\ alive = {"self"}
  /\ suspect = {}
  /\ dead = {}
  /\ lastPing = {}
  /\ incarnation = ["self" |-> 0]
  /\ pingTargets = ["self" |-> {}]

Join(n) ==
  /\ n \notin members
  /\ members' = members \cup {n}
  /\ alive' = alive \cup {n}
  /\ suspect' = suspect
  /\ dead' = dead
  /\ lastPing' = lastPing
  /\ incarnation' = [incarnation EXCEPT ![n] = 0]
  /\ pingTargets' = [pingTargets EXCEPT !["self"] = @ \cup {n}]

PingReq(src, dst) ==
  /\ src \in alive
  /\ dst \in members
  /\ src # dst
  /\ lastPing' = [lastPing EXCEPT ![<<src, dst>>] = 0]
  /\ members' = members
  /\ alive' = alive
  /\ suspect' = suspect
  /\ dead' = dead
  /\ incarnation' = incarnation
  /\ pingTargets' = pingTargets

PingAck(src, dst) ==
  /\ src \in alive
  /\ dst \in alive
  /\ src # dst
  /\ lastPing[<<dst, src>>] \in DOMAIN lastPing
  /\ lastPing' = [lastPing EXCEPT ![<<dst, src>>] = 0]
  /\ members' = members
  /\ alive' = alive
  /\ suspect' = suspect
  /\ dead' = dead
  /\ incarnation' = incarnation
  /\ pingTargets' = pingTargets

Suspect(n) ==
  /\ n \in alive
  /\ n # "self"
  /\ lastPing[<<n, "self">>] \in DOMAIN lastPing
  /\ alive' = alive \ {n}
  /\ suspect' = suspect \cup {n}
  /\ members' = members
  /\ dead' = dead
  /\ lastPing' = lastPing
  /\ incarnation' = incarnation
  /\ pingTargets' = pingTargets

ConfirmDead(n) ==
  /\ n \in suspect
  /\ n # "self"
  /\ lastPing[<<n, "self">>] \in DOMAIN lastPing
  /\ suspect' = suspect \ {n}
  /\ dead' = dead \cup {n}
  /\ members' = members
  /\ alive' = alive
  /\ lastPing' = lastPing
  /\ incarnation' = incarnation
  /\ pingTargets' = pingTargets

Leave(n) ==
  /\ n \in members
  /\ members' = members \ {n}
  /\ alive' = alive \ {n}
  /\ suspect' = suspect \ {n}
  /\ dead' = dead \ {n}
  /\ lastPing' = lastPing
  /\ incarnation' = incarnation
  /\ pingTargets' = [pingTargets EXCEPT !["self"] = @ \ {n}]

Stutter ==
  /\ UNCHANGED <<members, alive, suspect, dead, pingTargets,
                  lastPing, incarnation>>

Next ==
  \/ \E n : Join(n)
  \/ \E src, dst : PingReq(src, dst)
  \/ \E src, dst : PingAck(src, dst)
  \/ \E n : Suspect(n)
  \/ \E n : ConfirmDead(n)
  \/ \E n : Leave(n)
  \/ Stutter

Spec == Init /\ [][Next]_<<members, alive, suspect, dead, pingTargets,
                         lastPing, incarnation>>

NoSplitBrain ==
  /\ alive \cap dead = {}

SelfExclusion ==
  /\ "self" \notin suspect
  /\ "self" \notin dead

EventualConsistency ==
  /\ suspect \cap alive = {}

THEOREM Spec => []TypeInvariant
THEOREM Spec => []NoSplitBrain
THEOREM Spec => []SelfExclusion
THEOREM Spec => []EventualConsistency

===============================================================================
\* TLC Model Checker Configuration
\*
\*   INIT Init
\*   NEXT Next
\*   INVARIANTS TypeInvariant, NoSplitBrain, SelfExclusion, EventualConsistency
\*
\*   CONSTANTS
\*     Node = {"self", "n1", "n2"}
\*
\*   SPECIFICATION Spec
\*
\*   Properties to verify:
\*     NoSplitBrain     — alive and dead are always disjoint
\*     SelfExclusion    — "self" never enters suspect or dead
\*     EventualConsistency — suspect and alive are always disjoint
===============================================================================
