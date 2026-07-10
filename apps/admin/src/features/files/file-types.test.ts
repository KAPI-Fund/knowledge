import { describe, expect, it } from "vitest";

import {
  getCodeLanguage,
  getFileCategory,
  getFileExtension,
  hasServerTextContent,
  isBinary,
  isExtractedTextPreviewFile,
  isTextReadable,
} from "./file-types";

describe("file-types", () => {
  it("maps extensions to categories", () => {
    expect(getFileCategory("wiki/index.md")).toBe("markdown");
    expect(getFileCategory("agent-workspace/report/out.svg")).toBe("code");
    expect(getFileCategory("agent-workspace/diagram.mmd")).toBe("code");
    expect(getFileCategory("wiki/media/shot.PNG")).toBe("image");
    expect(getFileCategory("raw/sources/book.pdf")).toBe("pdf");
    expect(getFileCategory("raw/sources/table.csv")).toBe("data");
    expect(getFileCategory("raw/sources/talk.docx")).toBe("document");
    expect(getFileCategory("mystery.bin")).toBe("unknown");
  });

  it("classifies readability and binary-ness", () => {
    expect(isTextReadable("markdown")).toBe(true);
    expect(isTextReadable("data")).toBe(true);
    expect(isTextReadable("pdf")).toBe(false);
    expect(isBinary("image")).toBe(true);
    expect(isBinary("code")).toBe(false);
  });

  it("extracts file extensions from paths", () => {
    expect(getFileExtension("wiki/a/b.Md")).toBe("md");
    expect(getFileExtension("wiki/no-extension")).toBe("");
    expect(getFileExtension("wiki\\win\\path.txt")).toBe("txt");
  });

  it("limits extracted-text preview to office docs under raw/sources", () => {
    expect(isExtractedTextPreviewFile("raw/sources/deck.pptx")).toBe(true);
    expect(isExtractedTextPreviewFile("raw/sources/book.pdf")).toBe(false);
    expect(isExtractedTextPreviewFile("agent-workspace/deck.pptx")).toBe(false);
  });

  it("reports server text availability", () => {
    expect(hasServerTextContent("wiki/index.md")).toBe(true);
    expect(hasServerTextContent("agent-workspace/report.html")).toBe(true);
    expect(hasServerTextContent("raw/sources/talk.docx")).toBe(true);
    expect(hasServerTextContent("wiki/media/shot.png")).toBe(false);
    expect(hasServerTextContent("raw/sources/book.pdf")).toBe(false);
  });

  it("maps extensions to highlight languages", () => {
    expect(getCodeLanguage("a/b.tsx")).toBe("typescript");
    expect(getCodeLanguage("a/b.mmd")).toBe("mermaid");
    expect(getCodeLanguage("a/b.zig")).toBe("zig");
  });
});
