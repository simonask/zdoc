# zdocument

Binary JSON or XML style document/value library designed for zero-copy access.

`zdocument` is a node-based document format that can represent arbitrary
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
| KDL  | ✅               | ✅             |
| JSON | ✅[^comments]    | ✅[^kvargs][^dupes]    |
| YAML | ✅[^comments]    | ✅[^kvargs][^dupes]    |

Additionally, `zdocument::Document` can be serialized either as
non-human-readable flat data or into any structured format.

[^xml]: Same limitations as for KDL.
[^comments]: Comments will not be preserved.
[^kvargs]: Non-key-value node arguments will not be preserved.
[^dupes]: This format does not support duplicate keys in maps. The _last_
    property or node with a particular name will win.
