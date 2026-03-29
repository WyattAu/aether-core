"""Unit tests for LeaderElection — deterministic bully algorithm."""

import pytest

from server.cluster.election import LeaderElection
from server.cluster.node import ClusterNode, NodeStatus


def _make_node(node_id, status=NodeStatus.ALIVE, incarnation=0):
    return ClusterNode(
        node_id=node_id,
        host=f"10.0.0.{hash(node_id) % 256}",
        gossip_port=9000,
        api_port=8000,
        status=status,
        incarnation=incarnation,
    )


class TestElectionBasic:
    """Tests for basic election logic."""

    def test_single_node_elects_itself(self):
        """A single alive node should elect itself."""
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        leader = election.elect(members)
        assert leader == "node-a"
        assert election.is_leader is True

    def test_lowest_id_wins(self):
        """The alive node with the lowest lexicographic ID becomes leader."""
        election = LeaderElection(node_id="node-c")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
            "node-c": _make_node("node-c"),
        }
        leader = election.elect(members)
        assert leader == "node-a"

    def test_numeric_ids_sorted_lexicographically(self):
        """Numeric IDs are sorted lexicographically, not numerically."""
        election = LeaderElection(node_id="10")
        members = {
            "10": _make_node("10"),
            "2": _make_node("2"),
            "9": _make_node("9"),
        }
        leader = election.elect(members)
        # Lexicographic: "10" < "2" < "9"
        assert leader == "10"

    def test_uuid_ids_sorted_lexicographically(self):
        """UUID-style IDs are sorted lexicographically."""
        election = LeaderElection(node_id="node-z")
        members = {
            "node-m": _make_node("node-m"),
            "node-a": _make_node("node-a"),
            "node-z": _make_node("node-z"),
        }
        leader = election.elect(members)
        assert leader == "node-a"

    def test_empty_members_no_leader(self):
        """No members means no leader."""
        election = LeaderElection(node_id="node-a")
        leader = election.elect({})
        assert leader is None
        assert election.is_leader is False

    def test_all_dead_no_leader(self):
        """All dead nodes means no leader."""
        election = LeaderElection(node_id="node-a")
        members = {
            "node-a": _make_node("node-a", status=NodeStatus.DEAD),
            "node-b": _make_node("node-b", status=NodeStatus.DEAD),
        }
        leader = election.elect(members)
        assert leader is None

    def test_suspect_nodes_excluded(self):
        """SUSPECT nodes are excluded from election."""
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a", status=NodeStatus.SUSPECT),
            "node-b": _make_node("node-b", status=NodeStatus.ALIVE),
        }
        leader = election.elect(members)
        assert leader == "node-b"

    def test_leaving_nodes_excluded(self):
        """LEAVING nodes are excluded from election."""
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a", status=NodeStatus.LEAVING),
            "node-b": _make_node("node-b", status=NodeStatus.ALIVE),
        }
        leader = election.elect(members)
        assert leader == "node-b"

    def test_joining_nodes_included(self):
        """JOINING nodes are included in election."""
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a", status=NodeStatus.JOINING),
            "node-b": _make_node("node-b", status=NodeStatus.ALIVE),
        }
        leader = election.elect(members)
        assert leader == "node-a"


class TestElectionChanges:
    """Tests for leader changes on membership updates."""

    def test_leader_changes_on_node_join(self):
        """Adding a lower-ID node should trigger leadership change."""
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        election.elect(members)
        assert election.is_leader is True

        # Lower ID joins
        members["aaa"] = _make_node("aaa")
        election.elect(members)
        assert election.leader_id == "aaa"
        assert election.is_leader is False

    def test_leader_changes_on_node_failure(self):
        """Leader failure should re-elect next lowest."""
        election = LeaderElection(node_id="node-c")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
            "node-c": _make_node("node-c"),
        }
        election.elect(members)
        assert election.leader_id == "node-a"

        # Leader dies
        members["node-a"] = _make_node("node-a", status=NodeStatus.DEAD)
        election.elect(members)
        assert election.leader_id == "node-b"

    def test_leader_changes_on_node_leave(self):
        """Leader leaving should re-elect next lowest."""
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
        }
        election.elect(members)
        assert election.leader_id == "node-a"

        del members["node-a"]
        election.elect(members)
        assert election.leader_id == "node-b"
        assert election.is_leader is True

    def test_no_change_if_leader_still_alive(self):
        """Re-election with same membership should not change leader."""
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        election.elect(members)

        callback_calls = []
        election.on_leader_change(lambda old, new: callback_calls.append((old, new)))

        # Re-elect with same state
        election.elect(members)
        assert election.leader_id == "node-a"
        assert len(callback_calls) == 0  # No change

    def test_leader_recovered(self):
        """If a failed leader comes back alive, it regains leadership."""
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
        }
        election.elect(members)
        assert election.leader_id == "node-a"

        # Leader dies
        members["node-a"] = _make_node("node-a", status=NodeStatus.DEAD)
        election.elect(members)
        assert election.leader_id == "node-b"

        # Leader recovers
        members["node-a"] = _make_node("node-a", status=NodeStatus.ALIVE)
        election.elect(members)
        assert election.leader_id == "node-a"


class TestElectionCount:
    """Tests for election counting."""

    def test_election_count_increments(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        assert election.election_count == 0

        election.elect(members)
        assert election.election_count == 1

        election.elect(members)
        assert election.election_count == 2

    def test_get_status(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        election.elect(members)

        status = election.get_status()
        assert status["leader_id"] == "node-a"
        assert status["is_leader"] is True
        assert status["election_count"] == 1
        assert "leader_incarnation" in status


class TestStepDown:
    """Tests for voluntary leader step-down."""

    def test_step_down_when_leader(self):
        election = LeaderElection(node_id="node-a")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
        }
        election.elect(members)
        assert election.is_leader is True

        result = election.step_down()
        assert result is None  # step_down clears, returns None
        assert election.is_leader is False
        # Re-elect: node-a is stepped down, so node-b wins
        new_leader = election.elect(members)
        assert new_leader == "node-b"
        assert election.is_leader is False
        assert election.is_leader is False

    def test_step_down_when_not_leader(self):
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
        }
        election.elect(members)

        result = election.step_down()
        # Not the leader, returns current leader
        assert result == "node-a"
        assert election.is_leader is False

    def test_step_down_no_peers(self):
        """Stepping down with no peers means no leader."""
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        election.elect(members)

        election.step_down()
        new_leader = election.elect(members)
        # Only node, and it stepped down — no leader
        assert new_leader is None
        assert election.is_leader is False


class TestForceLeader:
    """Tests for forced leader election (admin operation)."""

    def test_force_leader_success(self):
        election = LeaderElection(node_id="node-a")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
            "node-c": _make_node("node-c"),
        }
        election.elect(members)
        assert election.leader_id == "node-a"

        success = election.force_leader("node-c", members)
        assert success is True
        assert election.leader_id == "node-c"
        assert election.is_leader is False

    def test_force_leader_dead_node_rejected(self):
        election = LeaderElection(node_id="node-a")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b", status=NodeStatus.DEAD),
        }
        # First, elect a leader
        election.elect(members)
        assert election.leader_id == "node-a"
        # Force a dead node — should be rejected, leader unchanged
        success = election.force_leader("node-b", members)
        assert success is False
        assert election.leader_id == "node-a"

    def test_force_leader_nonexistent_rejected(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}
        success = election.force_leader("nonexistent", members)
        assert success is False

    def test_force_leader_increments_election_count(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a"), "node-b": _make_node("node-b")}
        election.elect(members)
        assert election.election_count == 1

        election.force_leader("node-b", members)
        assert election.election_count == 2


class TestLeaderChangeCallback:
    """Tests for the on_leader_change callback."""

    def test_callback_on_first_election(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}

        calls = []
        election.on_leader_change(lambda old, new: calls.append((old, new)))

        election.elect(members)
        assert len(calls) == 1
        assert calls[0] == (None, "node-a")

    def test_callback_on_leader_change(self):
        election = LeaderElection(node_id="node-b")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
        }
        election.elect(members)

        calls = []
        election.on_leader_change(lambda old, new: calls.append((old, new)))

        # Kill leader
        members["node-a"] = _make_node("node-a", status=NodeStatus.DEAD)
        election.elect(members)
        assert len(calls) == 1
        assert calls[0] == ("node-a", "node-b")

    def test_callback_exception_caught(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a")}

        def bad_callback(old, new):
            raise RuntimeError("boom")

        election.on_leader_change(bad_callback)
        # Should not raise
        election.elect(members)
        assert election.leader_id == "node-a"

    def test_callback_on_force_leader(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a"), "node-b": _make_node("node-b")}
        election.elect(members)

        calls = []
        election.on_leader_change(lambda old, new: calls.append((old, new)))

        election.force_leader("node-b", members)
        assert len(calls) == 1
        assert calls[0] == ("node-a", "node-b")

    def test_callback_on_step_down(self):
        election = LeaderElection(node_id="node-a")
        members = {"node-a": _make_node("node-a"), "node-b": _make_node("node-b")}
        election.elect(members)

        calls = []
        election.on_leader_change(lambda old, new: calls.append((old, new)))

        election.step_down()
        # step_down clears leader but doesn't trigger callback
        assert len(calls) == 0

        # Re-elect: node-a stepped down, node-b becomes leader
        election.elect(members)
        assert len(calls) == 1
        assert calls[0] == ("node-a", "node-b")


class TestThreadSafety:
    """Thread safety tests."""

    def test_concurrent_elections(self):
        import threading
        election = LeaderElection(node_id="node-a")
        members = {
            "node-a": _make_node("node-a"),
            "node-b": _make_node("node-b"),
            "node-c": _make_node("node-c"),
        }
        errors = []

        def elect_n(n):
            try:
                for _ in range(100):
                    election.elect(members)
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=elect_n, args=(t,)) for t in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        assert election.election_count == 400
        assert election.leader_id == "node-a"
