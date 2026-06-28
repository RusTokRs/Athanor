# Projector Support

Shared implementation support for filesystem-backed Athanor projectors.

This crate provides the canonical projection payload shape, shared attachment indexes, collision-free generated filenames, staged directory replacement, immutable directory publication, and replaceable pointer files used by the Markdown wiki, HTML report, and coordinated generation service. It also exposes file-write helpers for both one-off outputs and high-volume page writers that pre-create their parent directories. It is not a projector or canonical store itself.
