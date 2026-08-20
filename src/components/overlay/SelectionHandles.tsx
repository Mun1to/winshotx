import type { Rect } from "../../lib/types";

export type HandleId = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

const HANDLES: { id: HandleId; cursor: string; fx: number; fy: number }[] = [
  { id: "nw", cursor: "nwse-resize", fx: 0, fy: 0 },
  { id: "n", cursor: "ns-resize", fx: 0.5, fy: 0 },
  { id: "ne", cursor: "nesw-resize", fx: 1, fy: 0 },
  { id: "e", cursor: "ew-resize", fx: 1, fy: 0.5 },
  { id: "se", cursor: "nwse-resize", fx: 1, fy: 1 },
  { id: "s", cursor: "ns-resize", fx: 0.5, fy: 1 },
  { id: "sw", cursor: "nesw-resize", fx: 0, fy: 1 },
  { id: "w", cursor: "ew-resize", fx: 0, fy: 0.5 },
];

interface Props {
  rect: Rect;
  onGrab: (handle: HandleId, event: React.PointerEvent) => void;
}

export function SelectionHandles({ rect, onGrab }: Props) {
  return (
    <>
      {HANDLES.map((h) => (
        <div
          key={h.id}
          onPointerDown={(e) => {
            e.stopPropagation();
            onGrab(h.id, e);
          }}
          style={{
            left: rect.x + rect.width * h.fx,
            top: rect.y + rect.height * h.fy,
            cursor: h.cursor,
          }}
          className="absolute z-30 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-[1.5px] border-blue-500 bg-white shadow-[0_1px_4px_rgba(0,0,0,0.5)] transition-transform hover:scale-125"
        />
      ))}
    </>
  );
}
