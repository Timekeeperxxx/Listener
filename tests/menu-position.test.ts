import { describe, expect, it } from 'vitest'
import { positionMenu, positionSubmenu } from '../src/lib/menu-position'
const viewport = { width: 1280, height: 840 }, menu = { width: 216, height: 260 }
describe('context menu placement', () => {
  it('aligns the main menu with its triggering button', () => {
    expect(positionMenu({ left: 1160, right: 1190, top: 200, bottom: 230 }, menu, viewport)).toEqual({ left: 974, top: 235 })
  })
  it('opens upward at the bottom edge and stays inside the left edge', () => {
    expect(positionMenu({ left: 8, right: 38, top: 795, bottom: 825 }, menu, viewport)).toEqual({ left: 8, top: 530 })
  })
  it('flips a submenu left near the right edge and clamps its bottom', () => {
    expect(positionSubmenu({ left: 974, right: 1190, top: 535, bottom: 795 }, { left: 980, right: 1184, top: 760, bottom: 794 }, { width: 232, height: 180 }, viewport)).toEqual({ left: 737, top: 652, opensLeft: true })
  })
  it('opens a submenu right when there is room', () => {
    expect(positionSubmenu({ left: 200, right: 416, top: 100, bottom: 360 }, { left: 206, right: 410, top: 200, bottom: 234 }, { width: 232, height: 120 }, viewport)).toEqual({ left: 421, top: 194, opensLeft: false })
  })
})
