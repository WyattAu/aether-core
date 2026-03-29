"""Tests for ClusterConfig."""

from server.cluster.config import ClusterConfig


class TestClusterConfigDefaults:

    def test_disabled_by_default(self):
        config = ClusterConfig()
        assert config.enabled is False

    def test_default_node_id_empty(self):
        config = ClusterConfig()
        assert config.node_id == ""

    def test_default_seed_nodes_empty(self):
        config = ClusterConfig()
        assert config.seed_nodes == []

    def test_default_gossip_port(self):
        config = ClusterConfig()
        assert config.gossip_port == 7946

    def test_default_virtual_nodes(self):
        config = ClusterConfig()
        assert config.virtual_nodes == 150

    def test_default_transport(self):
        config = ClusterConfig()
        assert config.transport == "http"


class TestClusterConfigCustom:

    def test_custom_values(self):
        config = ClusterConfig(
            enabled=True,
            node_id="node-1",
            seed_nodes=["10.0.0.1:7946", "10.0.0.2:7946"],
            gossip_port=9000,
            virtual_nodes=200,
        )
        assert config.enabled is True
        assert config.node_id == "node-1"
        assert len(config.seed_nodes) == 2
        assert config.virtual_nodes == 200
