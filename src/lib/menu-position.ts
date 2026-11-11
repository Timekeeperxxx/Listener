interface Box { left: number; right: number; top: number; bottom: number }
interface Size { width: number; height: number }
const margin = 8, gap = 5
const clamp = (value: number, limit: number) => Math.max(margin, Math.min(value, limit - margin))

export function positionMenu(anchor: Box, menu: Size, viewport: Size) {
  const below = anchor.bottom + gap
  return {
    left: clamp(anchor.right - menu.width, viewport.width - menu.width),
    top: clamp(below + menu.height <= viewport.height - margin ? below : anchor.top - menu.height - gap, viewport.height - menu.height),
  }
}

export function positionSubmenu(parent: Box, item: Box, menu: Size, viewport: Size) {
  const opensLeft = parent.right + gap + menu.width > viewport.width - margin
  return {
    left: clamp(opensLeft ? parent.left - menu.width - gap : parent.right + gap, viewport.width - menu.width),
    top: clamp(item.top - 6, viewport.height - menu.height),
    opensLeft,
  }
}
