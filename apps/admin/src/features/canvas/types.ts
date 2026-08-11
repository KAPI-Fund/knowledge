import { z } from "zod";

export const canvasNodeSchema = z.object({
  id: z.string(),
  type: z.enum(["note", "url", "kb", "ai_analyze", "ai_image", "search", "html"]),
  x: z.number(),
  y: z.number(),
  w: z.number().default(280),
  h: z.number().default(160),
  data: z.record(z.string(), z.unknown()).default({}),
});
export type CanvasNode = z.infer<typeof canvasNodeSchema>;

export const canvasEdgeSchema = z.object({
  id: z.string(),
  source: z.string(),
  target: z.string(),
  sourceHandle: z.string().optional(),
  targetHandle: z.string().optional(),
  kind: z.string().optional(),
});
export type CanvasEdge = z.infer<typeof canvasEdgeSchema>;

export const viewportSchema = z.object({ x: z.number(), y: z.number(), zoom: z.number() });
export type Viewport = z.infer<typeof viewportSchema>;

export const canvasDocumentSchema = z.object({
  nodes: z.array(canvasNodeSchema).default([]),
  edges: z.array(canvasEdgeSchema).default([]),
  viewport: viewportSchema.default({ x: 0, y: 0, zoom: 1 }),
});
export type CanvasDocument = z.infer<typeof canvasDocumentSchema>;

export const canvasResponseSchema = z.object({
  id: z.string(),
  title: z.string(),
  document: canvasDocumentSchema,
  created_at: z.string(),
  updated_at: z.string(),
});
export type CanvasResponse = z.infer<typeof canvasResponseSchema>;

export const canvasSummarySchema = z.object({
  id: z.string(),
  title: z.string(),
  updated_at: z.string(),
});
export type CanvasSummary = z.infer<typeof canvasSummarySchema>;
