# Open Source Credits and Prior Art

Kosh is built upon a foundation of incredible Open Source Software. We are deeply grateful to the authors and maintainers of these projects.

## Core Parsing Engine
- **[Tree-sitter](https://github.com/tree-sitter/tree-sitter)**: A parser generator tool and an incremental parsing library. Created by Max Brunsfeld.

## Language Grammars
We use the following Tree-sitter grammars for symbol extraction:
- **[tree-sitter-rust](https://github.com/tree-sitter/tree-sitter-rust)**: The Tree-sitter authors.
- **[tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java)**: The Tree-sitter authors.
- **[tree-sitter-kotlin](https://github.com/fwcd/tree-sitter-kotlin)**: Maintained by [fwcd](https://github.com/fwcd) and community contributors.
- **[tree-sitter-python](https://github.com/tree-sitter/tree-sitter-python)**: Max Brunsfeld and the Tree-sitter authors.
- **[tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript)**: The Tree-sitter authors.
- **[tree-sitter-javascript](https://github.com/tree-sitter/tree-sitter-javascript)**: Max Brunsfeld and the Tree-sitter authors.
- **[tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go)**: Max Brunsfeld and the Tree-sitter authors.
- **[tree-sitter-cpp](https://github.com/tree-sitter/tree-sitter-cpp)**: Max Brunsfeld and the Tree-sitter authors.
- **[tree-sitter-dart](https://github.com/User-G/tree-sitter-dart)**: Maintained by [User-G](https://github.com/User-G).

## Symbol Queries (Prior Art)
Our symbol extraction queries (`tags.scm`) are inspired by or derived from the work of:
- **[GitHub Code Navigation](https://github.com/github/semantic)**: For standardized `tags.scm` query patterns.
- **[Helix Editor](https://github.com/helix-editor/helix)**: For high-quality, community-maintained tree-sitter queries.
- **[nvim-treesitter](https://github.com/nvim-treesitter/nvim-treesitter)**: For extensive community testing and refinement of language-specific queries.

## Protocols and Standards
- **[SCIP](https://github.com/sourcegraph/scip)**: Developed by [Sourcegraph](https://sourcegraph.com) as a modern successor to LSIF, providing inspiration for Kosh's long-term semantic indexing goals.

---
*Note: If you believe your project should be credited here or if information is incorrect, please open an issue.*
