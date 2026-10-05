# 玻璃压花高度图

2026-10-04 使用 Codex 内置 imagegen 生成。生成图像作为灰度高度输入，并非实测玻璃表面的扫描数据。参考用户的花格与冰花玻璃照片来描述结构，不使用照片中的水印或背景像素。

原图保存在本目录。2026-10-05 修正运行时处理：复古花格取内部大花中心至右下小花中心的象限，镜像重建一个对称周期单元，使角落花心位于拼接交点；保留原 PNG 作为生成来源。冰花仍缩小并混合对边窄条。两者经 1.2 像素平滑及周期低通，再用周期中心差分生成法线；法线 mipmap 按坡度平均。生成图仍可能包含明暗塑形，这是艺术化起伏近似。运行时高度拼接预览由测试导出到 `outputs/pattern-preview/quatrefoil-periodic.png`。

## ornamental

Use case: stylized-concept. Asset type: grayscale HEIGHT MAP texture for embossed patterned glass, NOT a photograph or rendered glass. Make a seamless square repeating ornamental quatrefoil glass pattern inspired by vintage pressed glass: elegant interlocking four-lobed medallions with double beaded borders, small eight-petal rosettes in gaps, delicate stippled interiors. 3 by 3 evenly repeated medallions across the square. Orthographic flat top-down scalar displacement map: neutral middle-gray base, brighter smooth rounded embossed ridges, darker shallow grooves. No directional lighting, no shadows, no reflections, no background scene, no perspective, no writing, logos, watermark, border or colored pixels. Crisp but smoothly rounded ridge gradients, readable silhouette, uniform relief range, seamless wrapping at all edges. Output one square grayscale image.

## fern

Use case: stylized-concept. Asset type: grayscale HEIGHT MAP texture for fern ice-flower embossed glass, NOT a photo or rendering of glass. Make a seamless square of delicate frost fern leaves and flowing ice-flower botanical sprays inspired by vintage fern-pattern pressed window glass. Several curving stems branching into elegant slender paired feather-like leaflets, varied orientation and scale, layered natural botanical rhythm, generous quiet gaps, subtle fine leaf veins. Orthographic flat top-down scalar displacement map: middle-gray background, lighter smoothly rounded raised stems and leaflets, darker shallow surrounding grooves. No directional lighting, no shadows, no reflections, no background scenery, no perspective, no writing, logos, watermarks, border or color. Continuous smooth bevel gradients, coherent forms, avoid photographic luminance texture. Seamless wrapping all edges. Output one square grayscale image.
