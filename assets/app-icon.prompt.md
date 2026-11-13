# Listener 声波图标

Mac 图标和网页页签使用 [iPhone 默认图标](../ios/ListenerNative/Assets.xcassets/AppIcon.appiconset/Light.png) 的六条蓝色声波。Mac 导出仅加入圆角遮罩和透明留白。更新 iPhone 图稿后，运行 `swift scripts/generate-icons.swift` 重新导出 Mac 的 PNG 和 ICNS。

## 早期图稿

`app-icon-source.png` 是早期由内置 imagegen 生成的图稿，作为设计记录保留。

保留六条线的数量、排列和长短节奏；以蓝紫色珐琅与玻璃光泽、珍珠白底增加立体质感。最终导出使用 macOS 圆角遮罩、透明留白和不透明内底，避免透明生成区域的零散像素进入图标。原始图稿未改动。

## 生成提示

```text
Use case: style-transfer. Asset type: final macOS app icon for Listener, a refined music player. Edit target: the attached six-bar sound-wave mark. Transform this plain mark into a beautifully crafted, sculptural app icon while preserving its identity. Exactly SIX parallel upright rounded bars, clearly separated, centered about one horizontal axis. From left to right their approximate relative lengths are 22, 64, 100, 43, 75, 22; third bar tallest. Preserve this recognisable uneven rhythm, with even horizontal spacing. Render the bars as substantial satin enamel / softly polished glass forms in rich slate blue, indigo and delicate periwinkle highlights; restrained dimensional bevels and reflected light, front-on, no perspective tilt. Put the mark in the center of a warm pearl-white macOS continuous-corner rounded square tile, with subtly cool inner shading and immaculate soft edges. Gentle upper-left studio lighting, controlled depth, sophisticated material craftsmanship, elegant visual quiet. The mark should be bold, legible and high contrast even at 32 pixels, occupy about 54% of the tile width and 64% of its height. The rounded square occupies about 84% of the square image canvas, with generous consistent transparent space around it on all sides. Actual transparency outside the tile, including its rounded corners. Single centered 1024-by-1024 icon composition. No text, no lettering, no extra symbols, no outlines around the tile, no music notes, no circles, no watermark, no busy texture, no sparkles, no oversized shadow, no mockup scene. This is the finished icon asset, not a presentation of the icon.
```

## 修整提示

```text
Use case: precise-object-edit. This is final production cleanup of the provided macOS sound-wave app icon. Keep the six sculpted blue-periwinkle enamel bars, exact lengths, spacing, material, soft lighting and pearl-white rounded tile unchanged. Change ONLY the outer silhouette and transparent area: make the continuous-corner rounded-square edge smooth, symmetrical, clean and perfectly antialiased. Remove every stray white fleck, blue speck, detached pixel, halo and rough contour outside the rounded tile, especially above and below it. Outside the tile must be completely and genuinely transparent, zero opaque or semi-opaque debris. Inside the tile should be fully opaque. Keep generous even transparent margins on all four sides, tile occupies about 82% of square canvas. No cast shadow outside the tile. Do not add objects, text, graphics or texture. Deliver one finished square macOS application icon asset.
```
