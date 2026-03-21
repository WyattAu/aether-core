"""
Schema Registry for Event Validation

Provides schema registry capabilities for event validation, schema evolution,
 and compatibility checking.

Example:
    from aether_sdk.event import SchemaRegistry, InMemorySchemaRegistry, Schema
    
    # Create registry
    registry = InMemorySchemaRegistry()
    
    # Register a schema
    schema = Schema(
        name="UserCreated",
        type="json",
        definition={
            "type": "object",
            "properties": {
                "userId": {"type": "string"},
                "email": {"type": "string"}
            },
            "required": ["userId", "email"]
        }
    )
    await registry.register("UserCreated", schema)
    
    # Validate an event
    event = {"userId": "123", "email": "user@example.com"}
    valid = await registry.validate("UserCreated", event)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime
from typing import (
    Any,
    Dict,
    List,
    Optional,
    Set,
    Union,
)
from abc import ABC, abstractmethod
from enum import Enum
import asyncio
import json
import uuid

from ..exceptions import AetherError


class Compatibility(Enum):
    """
    Schema compatibility levels for version evolution.
    
    - BACKWARD: New schema can read old data
    - FORWARD: Old schema can read new data  
    - FULL: Both backward and forward compatible
    - NONE: No compatibility (breaking change)
    """
    NONE = "none"
    BACKWARD = "backward"
    FORWARD = "forward"
    FULL = "full"


@dataclass
class SchemaVersion:
    """
    Represents a versioned schema in the registry.
    """
    version: str  # Semantic version like "1.0.0"
    schema_id: str
    definition: Dict[str, Any]
    created_at: datetime = field(default_factory=datetime.utcnow)
    deprecated: bool = False
    compatibility: Compatibility = Compatibility.BACKWARD
    
    def __str__(self) -> str:
        return f"{self.schema_id}@{self.version}"


    
    @property
    def major_version(self) -> int:
        """Extract major version number."""
        return int(self.version.split('.')[0])


@dataclass
class Schema:
    """
    Schema definition for event validation.
    """
    name: str
    type: str  # json, avro, protobuf, custom
    definition: Dict[str, Any]
    description: str = ""
    version: str = "1.0.0"
    namespace: Optional[str] = None
    owner: Optional[str] = None
    created_at: datetime = field(default_factory=datetime.utcnow)
    updated_at: datetime = field(default_factory=datetime.utcnow)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize schema to dictionary."""
        return {
            "name": self.name,
            "type": self.type,
            "definition": self.definition,
            "description": self.description,
            "version": self.version,
            "namespace": self.namespace,
            "owner": self.owner,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Schema:
        """Deserialize schema from dictionary."""
        return cls(
            name=data["name"],
            type=data["type"],
            definition=data["definition"],
            description=data.get("description", ""),
            version=data.get("version", "1.0.0"),
            namespace=data.get("namespace"),
            owner=data.get("owner"),
            created_at=datetime.fromisoformat(data["created_at"]) if "created_at" in data else datetime.utcnow(),
            updated_at=datetime.fromisoformat(data["updated_at"]) if "updated_at" in data else datetime.utcnow(),
        )


class SchemaError(AetherError):
    """Raised when schema validation fails."""
    
    def __init__(self, message: str, schema_name: Optional[str] = None):
        self.schema_name = schema_name
        super().__init__(message)


class SchemaValidator(ABC):
    """
    Abstract base class for schema validators.
    
    Different validators can be implemented for different
    schema types (JSON Schema, Avro, Protobuf, etc.)
    """
    
    @abstractmethod
    def validate(self, data: Any, schema: Schema) -> List[str]:
        """
        Validate data against a schema.
        
        Args:
            data: The data to validate
            schema: The schema to validate against
        
        Returns:
            List of validation errors (empty if valid)
        """
        pass
    
    @abstractmethod
    def check_compatibility(
        self,
        old_schema: Schema,
        new_schema: Schema
    ) -> Compatibility:
        """
        Check compatibility between two schema versions.
        
        Args:
            old_schema: The current schema
            new_schema: The proposed new schema
        
        Returns:
            Compatibility level between the schemas
        """
        pass


class JsonSchemaValidator(SchemaValidator):
    """
    JSON Schema validator implementation.
    """
    
    def validate(self, data: Any, schema: Schema) -> List[str]:
        """Validate data against JSON Schema."""
        if schema.type != "json":
            return [f"Expected JSON schema, got {schema.type}"]
        
        definition = schema.definition
        errors = []
        
        # Simple JSON Schema validation
        if "required" in definition:
            for field in definition["required"]:
                if field not in data:
                    errors.append(f"Missing required field: {field}")
        
        if "properties" in definition:
            for prop_name, prop_def in definition["properties"].items():
                if prop_name in data:
                    prop_type = prop_def.get("type")
                    if prop_type:
                        if not self._check_type(data[prop_name], prop_type):
                            errors.append(
                                f"Field '{prop_name}' has wrong type: expected {prop_type}"
                            )
        
        return errors
    
    def _check_type(self, value: Any, expected_type: str) -> bool:
        """Check if value matches expected JSON Schema type."""
        type_map = {
            "string": str,
            "number": (int, float),
            "integer": int,
            "boolean": bool,
            "array": list,
            "object": dict,
            "null": type(None),
        }
        
        expected = type_map.get(expected_type)
        if expected is None:
            return True  # Unknown type, skip validation
        
        # Handle number/integer specially
        if expected_type == "number":
            return isinstance(value, (int, float))
        if expected_type == "integer":
            return isinstance(value, int) and not isinstance(value, bool)
        
        return isinstance(value, expected)
    
    def check_compatibility(
        self,
        old_schema: Schema,
        new_schema: Schema
    ) -> Compatibility:
        """Check JSON Schema compatibility."""
        if old_schema.type != new_schema.type:
            return Compatibility.NONE
        
        old_def = old_schema.definition
        new_def = new_schema.definition
        
        # Check required fields - adding required fields is breaking
        old_required = set(old_def.get("required", []))
        new_required = set(new_def.get("required", []))
        
        # New required fields = breaking change
        added_required = new_required - old_required
        if added_required:
            return Compatibility.NONE
        
        # Check property types - changing types is breaking
        old_props = old_def.get("properties", {})
        new_props = new_def.get("properties", {})
        
        for prop_name, old_prop in old_props.items():
            new_prop = new_props.get(prop_name)
            if new_prop and old_prop.get("type") != new_prop.get("type"):
                return Compatibility.NONE
        
        # Adding optional fields = backward compatible
        # Removing optional fields = forward compatible
        # No changes = full compatible
        
        if not new_required and set(new_props.keys()) >= set(old_props.keys()):
            return Compatibility.BACKWARD
        elif new_required or set(new_props.keys()) < set(old_props.keys()):
            return Compatibility.FORWARD
        else:
            return Compatibility.FULL


class SchemaRegistry(ABC):
    """
    Abstract base class for schema registries.
    
    Provides schema registration, validation, and versioning.
    """
    
    @abstractmethod
    async def register(
        self,
        name: str,
        schema: Schema,
        compatibility: Compatibility = Compatibility.BACKWARD
    ) -> SchemaVersion:
        """
        Register a new schema or schema version.
        
        Args:
            name: Schema name
            schema: Schema definition
            compatibility: Expected compatibility level
        
        Returns:
            The created schema version
        """
        pass
    
    @abstractmethod
    async def get_schema(self, name: str, version: Optional[str] = None) -> Optional[Schema]:
        """
        Get a schema by name and optional version.
        
        Args:
            name: Schema name
            version: Optional specific version (latest if None)
        
        Returns:
            The schema or None if not found
        """
        pass
    
    @abstractmethod
    async def get_versions(self, name: str) -> List[SchemaVersion]:
        """
        Get all versions of a schema.
        
        Args:
            name: Schema name
        
        Returns:
            List of schema versions
        """
        pass
    
    @abstractmethod
    async def validate(self, name: str, data: Any, version: Optional[str] = None) -> bool:
        """
        Validate data against a schema.
        
        Args:
            name: Schema name
            data: Data to validate
            version: Optional specific version (latest if None)
        
        Returns:
            True if valid, raises SchemaError if invalid
        """
        pass
    
    @abstractmethod
    async def check_compatibility(
        self,
        name: str,
        old_version: str,
        new_version: str
    ) -> Compatibility:
        """
        Check compatibility between two schema versions.
        
        Args:
            name: Schema name
            old_version: Current version
            new_version: Proposed version
        
        Returns:
            Compatibility level
        """
        pass


class InMemorySchemaRegistry(SchemaRegistry):
    """
    In-memory implementation of SchemaRegistry for testing and development.
    """
    
    def __init__(self, validator: Optional[SchemaValidator] = None):
        self._schemas: Dict[str, List[SchemaVersion]] = {}
        self._validator = validator or JsonSchemaValidator()
        self._lock = asyncio.Lock()
    
    async def register(
        self,
        name: str,
        schema: Schema,
        compatibility: Compatibility = Compatibility.BACKWARD
    ) -> SchemaVersion:
        """Register a new schema or schema version."""
        async with self._lock:
            if name not in self._schemas:
                self._schemas[name] = []
            
            # Determine version number
            existing_versions = self._schemas[name]
            if existing_versions:
                last_version = existing_versions[-1].version
                major, minor, patch = map(int, last_version.split('.'))
                new_version = f"{major}.{minor}.{patch + 1}"
            else:
                new_version = "1.0.0"
            
            # Check compatibility with previous version
            if existing_versions:
                old_schema = Schema(
                    name=name,
                    type=schema.type,
                    definition=existing_versions[-1].definition
                )
                computed_compat = self._validator.check_compatibility(
                    old_schema, schema
                )
                if computed_compat == Compatibility.NONE and compatibility != Compatibility.NONE:
                    # Warning: breaking change detected but allowed
                    pass
            
            schema_version = SchemaVersion(
                version=new_version,
                schema_id=str(uuid.uuid4()),
                definition=schema.definition,
                compatibility=compatibility
            )
            
            self._schemas[name].append(schema_version)
            return schema_version
    
    async def get_schema(self, name: str, version: Optional[str] = None) -> Optional[Schema]:
        """Get a schema by name and optional version."""
        async with self._lock:
            if name not in self._schemas:
                return None
            
            versions = self._schemas[name]
            if not versions:
                return None
            
            if version:
                for sv in versions:
                    if sv.version == version:
                        return Schema(
                            name=name,
                            type="json",  # Assuming JSON for now
                            definition=sv.definition,
                            version=sv.version
                        )
                return None
            
            # Return latest version
            latest = versions[-1]
            return Schema(
                name=name,
                type="json",
                definition=latest.definition,
                version=latest.version
            )
    
    async def get_versions(self, name: str) -> List[SchemaVersion]:
        """Get all versions of a schema."""
        async with self._lock:
            return list(self._schemas.get(name, []))
    
    async def validate(self, name: str, data: Any, version: Optional[str] = None) -> bool:
        """Validate data against a schema."""
        schema = await self.get_schema(name, version)
        if schema is None:
            raise SchemaError(f"Schema not found: {name}", name)
        
        errors = self._validator.validate(data, schema)
        if errors:
            raise SchemaError(
                f"Validation failed: {'; '.join(errors)}",
                name
            )
        
        return True
    
    async def check_compatibility(
        self,
        name: str,
        old_version: str,
        new_version: str
    ) -> Compatibility:
        """Check compatibility between two schema versions."""
        old_schema = await self.get_schema(name, old_version)
        new_schema = await self.get_schema(name, new_version)
        
        if old_schema is None or new_schema is None:
            raise SchemaError(f"Schema version not found: {name}")
        
        return self._validator.check_compatibility(old_schema, new_schema)


__all__ = [
    "Compatibility",
    "SchemaVersion",
    "Schema",
    "SchemaError",
    "SchemaValidator",
    "JsonSchemaValidator",
    "SchemaRegistry",
    "InMemorySchemaRegistry",
]
