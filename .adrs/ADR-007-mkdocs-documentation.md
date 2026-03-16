# ADR-007: MkDocs for Documentation Site

## Status

Accepted

## Context
We needed a documentation platform that is easy to maintain, supports multiple output formats, and integrates well with GitHub.

## Decision

We chose **MkDocs with Material theme** for the Aether documentation site.

### Rationale

1. **Markdown-based**: Write docs in familiar Markdown format
2. **Material theme**: Modern, responsive design out of the box
3. **GitHub Pages compatible**: Easy deployment to GitHub Pages
4. **Search built-in**: Full-text search without external dependencies
5. **Versioning support**: Multiple versions via mike plugin

### Directory Structure

```
docs-site/
├── mkdocs.yml           # Configuration
└── docs/
    ├── index.md         # Home page
    ├── getting-started/ # Installation, quickstart, concepts
    ├── sdks/            # SDK documentation
    ├── examples/        # Example walkthroughs
    ├── architecture/    # System architecture
    └── api-reference.md # API reference
```

### Features Enabled

- Dark/light mode toggle
- Navigation tabs and sections
- Code copy buttons
- Code annotations
- Search with suggestions
- Git revision dates

## Consequences

### Positive
- Easy to contribute (just edit Markdown)
- Fast builds (static site)
- No backend required
- Great mobile experience
- Integrated with repo (docs live with code)

### Negative
- Limited dynamic content
- Build step required for preview
- Theme customization requires CSS/JS knowledge

## Alternatives Considered

1. **Docusaurus**
   - Rejected: More complex setup, React-based, overkill for our needs

2. **GitBook**
   - Rejected: Proprietary, less control, costs money for teams

3. **Sphinx**
   - Rejected: Python-centric, reStructuredText less familiar than Markdown

4. **Hugo**
   - Rejected: More complex theming, less built-in documentation features

## Related
- [MkDocs Documentation](https://www.mkdocs.org/)
- [Material for MkDocs](https://squidfunk.github.io/mkdocs-material/)
