# zdocument

Binary JSON or XML style document/value library designed for zero-copy access.

`zdocument` is a binary node-based document format that can represent arbitrary
structured data. It supports XML-style properties and attributes, as well as
JSON-like objects and arrays.

The format supports "zero-copy" reading from a buffer of bytes, and is highly
FFI-friendly. In particular, the format supports passing documents as a simple
memcpy into WASM memory, and WIT bindings are available.

The feature set is a superset of the [KDL](https://kdl.dev/) document language,
and KDL documents can be represented without data loss.

The following formats can be losslessly[^comments] converted to/from
`zdocument::Document`:

|      | `Into<Document>` | `From<Document` |
| ---- | ---------------- | --------------- |
| XML  | ✅[^xml]         | ✅             |
| KDL  | ✅[^comments]    | ✅             |
| JSON | ✅[^comments]    | ✅[^dupes]     |
| YAML | ✅[^comments]    | ✅[^dupes]     |
| TOML | ✅[^comments]    | ✅[^dupes]     |

Additionally, `zdocument::Document` can be serialized either as
non-human-readable flat data or into any structured format.

[^xml]: Same limitations as for KDL.
[^comments]: Comments will not be preserved.
[^dupes]: This format does not support duplicate keys in maps. The _last_
    property or node with a particular name will win.

## Features

1. Nodes in zdoc have an optional type, which logically corresponds to an XML
   element name, or a YAML tag. For serialized Rust enums, the node type field
   is the enum variant name.
2. Nodes in zdoc have an optional name, which logically corresponds to a JSON
   object key, or the key in a YAML mapping.
3. Nodes in zdoc have arguments, which is a list of key-value pairs, where the
   value is some primitive type (string, number, boolean, etc.). Arguments have
   optional names, corresponding to XML attribute names or JSON object/YAML
   mapping keys.
4. Named and unnamed nodes and arguments can be mixed freely. This may be used
   to represent things like nodes that have both a number of attributes and a
   number of "inner items".
5. Strings in zdoc are always UTF-8 encoded.
6. The binary representation of a document is a flat array of nodes which are
   laid out such that traversing the document has optimal cache locality.
7. The binary representation of a document can be validated in a single pass up
   front, which obviates the need to perform any validating while accessing the
   document, including bounds checks and UTF-8 validation.

## Comparison with other formats

### XML

1. Supports zero-copy reading from a buffer of bytes.
2. Supports unnamed elements and attributes.
3. Much more compact than XML, especially for large documents.

### JSON

1. Supports zero-copy reading from a buffer of bytes.
2. Supports unnamed elements and attributes. JSON requires that you choose
   between an array or an object.
3. Much more compact than JSON, especially for large documents.
4. Has a native way to indicate the "type" of a node (e.g. enum variant).
   Typically, type metadata in JSON has to be expressed as a special field
   containing the type tag.
5. Multiple entries in a node can have the same name (key).

### YAML

1. Supports zero-copy reading from a buffer of bytes.
2. Supports unnamed elements and attributes. YAML requires that you choose
   between a sequence or a mapping.
3. Much more compact than YAML, especially for large documents.
4. Multiple entries in a node can have the same name (key).

### TOML

1. Supports zero-copy reading from a buffer of bytes.
2. Supports unnamed elements and attributes. TOML requires that you choose
   between a table or an array.
3. Much more compact than TOML, especially for large documents.
4. Multiple entries in a node can have the same name (key).

### KDL

The feature set of zdoc is based on the features of KDL, so documents in the two
formats can be converted between the two formats without data loss.

1. Node names in zdoc are optional, where KDL conventionally uses "-" to
   indicate that a node is nameless (i.e., is a list item).
2. Zero-copy reading from a buffer of bytes.
3. Much more compact than KDL, especially for large documents.
