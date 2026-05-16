from aether_sdk.capabilities import Capability, CapabilitySet


class TestCapability:
    """Tests for Capability flag enum."""

    def test_capability_values_are_unique(self):
        """Each capability should have a unique value."""
        values = [cap.value for cap in Capability]
        assert len(values) == len(set(values)), "Capability values should be unique"

    def test_capability_network_outbound_exists(self):
        """NETWORK_OUTBOUND capability should exist."""
        assert hasattr(Capability, "NETWORK_OUTBOUND")
        assert Capability.NETWORK_OUTBOUND.value is not None

    def test_capability_network_inbound_exists(self):
        """NETWORK_INBOUND capability should exist."""
        assert hasattr(Capability, "NETWORK_INBOUND")
        assert Capability.NETWORK_INBOUND.value is not None

    def test_capability_state_read_exists(self):
        """STATE_READ capability should exist."""
        assert hasattr(Capability, "STATE_READ")
        assert Capability.STATE_READ.value is not None

    def test_capability_state_write_exists(self):
        """STATE_WRITE capability should exist."""
        assert hasattr(Capability, "STATE_WRITE")
        assert Capability.STATE_WRITE.value is not None

    def test_capability_fs_read_exists(self):
        """FS_READ capability should exist."""
        assert hasattr(Capability, "FS_READ")
        assert Capability.FS_READ.value is not None

    def test_capability_fs_write_exists(self):
        """FS_WRITE capability should exist."""
        assert hasattr(Capability, "FS_WRITE")
        assert Capability.FS_WRITE.value is not None

    def test_capability_actor_messaging_exists(self):
        """ACTOR_MESSAGING capability should exist."""
        assert hasattr(Capability, "ACTOR_MESSAGING")
        assert Capability.ACTOR_MESSAGING.value is not None

    def test_capability_count(self):
        """Should have expected number of capabilities."""
        expected_count = (
            13  # NETWORK_OUTBOUND, NETWORK_INBOUND, STATE_READ, STATE_WRITE,
        )
        # FS_READ, FS_WRITE, ACTOR_MESSAGING, LOG, TIME, RANDOM,
        # ENVIRONMENT, HTTP_CLIENT, HTTP_SERVER
        actual_count = len(list(Capability))
        assert (
            actual_count == expected_count
        ), f"Expected {expected_count} capabilities, got {actual_count}"


class TestCapabilitySet:
    """Tests for CapabilitySet class."""

    def test_empty_capability_set(self):
        """CapabilitySet should initialize empty."""
        caps = CapabilitySet()
        assert not caps.has(Capability.NETWORK_OUTBOUND)
        assert not caps.has(Capability.STATE_READ)

    def test_capability_set_with_initial_capabilities(self):
        """CapabilitySet should accept initial capabilities."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND, Capability.STATE_READ)
        assert caps.has(Capability.NETWORK_OUTBOUND)
        assert caps.has(Capability.STATE_READ)
        assert not caps.has(Capability.STATE_WRITE)

    def test_add_capability(self):
        """CapabilitySet.add should add a capability."""
        caps = CapabilitySet()
        caps.add(Capability.NETWORK_OUTBOUND)
        assert caps.has(Capability.NETWORK_OUTBOUND)

    def test_add_multiple_capabilities(self):
        """CapabilitySet should support adding multiple capabilities."""
        caps = CapabilitySet()
        caps.add(Capability.NETWORK_OUTBOUND)
        caps.add(Capability.STATE_READ)
        caps.add(Capability.STATE_WRITE)

        assert caps.has(Capability.NETWORK_OUTBOUND)
        assert caps.has(Capability.STATE_READ)
        assert caps.has(Capability.STATE_WRITE)
        assert not caps.has(Capability.FS_READ)

    def test_has_network_with_outbound(self):
        """has_network should return True with NETWORK_OUTBOUND."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        assert caps.has_network() is True

    def test_has_network_with_inbound(self):
        """has_network should return True with NETWORK_INBOUND."""
        caps = CapabilitySet(Capability.NETWORK_INBOUND)
        assert caps.has_network() is True

    def test_has_network_without_network_capabilities(self):
        """has_network should return False without network capabilities."""
        caps = CapabilitySet(Capability.STATE_READ)
        assert caps.has_network() is False

    def test_has_state_with_read(self):
        """has_state should return True with STATE_READ."""
        caps = CapabilitySet(Capability.STATE_READ)
        assert caps.has_state() is True

    def test_has_state_with_write(self):
        """has_state should return True with STATE_WRITE."""
        caps = CapabilitySet(Capability.STATE_WRITE)
        assert caps.has_state() is True

    def test_has_state_without_state_capabilities(self):
        """has_state should return False without state capabilities."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        assert caps.has_state() is False

    def test_has_returns_false_for_missing_capability(self):
        """has should return False for capability not in set."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        assert caps.has(Capability.STATE_READ) is False

    def test_add_same_capability_twice(self):
        """Adding same capability twice should not duplicate."""
        caps = CapabilitySet()
        caps.add(Capability.NETWORK_OUTBOUND)
        caps.add(Capability.NETWORK_OUTBOUND)
        assert caps.has(Capability.NETWORK_OUTBOUND)

    def test_all_capabilities_can_be_added(self):
        """All defined capabilities should be addable to CapabilitySet."""
        caps = CapabilitySet()
        for cap in Capability:
            caps.add(cap)
            assert caps.has(cap), f"Failed to add {cap}"
