import { TinyColor, TinyColor as TinyColor$1 } from "@ctrl/tinycolor";
import { getColors } from "theme-colors";
//#region src/color/color.ts
function isDarkColor(color) {
	return new TinyColor$1(color).isDark();
}
function isLightColor(color) {
	return new TinyColor$1(color).isLight();
}
//#endregion
//#region src/color/convert.ts
/**
* 将颜色转换为HSL格式。
*
* HSL是一种颜色模型，包括色相(Hue)、饱和度(Saturation)和亮度(Lightness)三个部分。
*
* @param {string} color 输入的颜色。
* @returns {string} HSL格式的颜色字符串。
*/
function convertToHsl(color) {
	const { a, h, l, s } = new TinyColor(color).toHsl();
	const hsl = `hsl(${Math.round(h)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%)`;
	return a < 1 ? `${hsl} ${a}` : hsl;
}
/**
* 将颜色转换为HSL CSS变量。
*
* 这个函数与convertToHsl函数类似，但是返回的字符串格式稍有不同，
* 以便可以作为CSS变量使用。
*
* @param {string} color 输入的颜色。
* @returns {string} 可以作为CSS变量使用的HSL格式的颜色字符串。
*/
function convertToHslCssVar(color) {
	const { a, h, l, s } = new TinyColor(color).toHsl();
	const hsl = `${Math.round(h)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%`;
	return a < 1 ? `${hsl} / ${a}` : hsl;
}
/**
* 将颜色转换为RGB颜色字符串
* TinyColor无法处理hsl内包含'deg'、'grad'、'rad'或'turn'的字符串
* 比如 hsl(231deg 98% 65%)将被解析为rgb(0, 0, 0)
* 这里在转换之前先将这些单位去掉
* @param str 表示HLS颜色值的字符串
* @returns 如果颜色值有效，则返回对应的RGB颜色字符串；如果无效，则返回rgb(0, 0, 0)
*/
function convertToRgb(str) {
	return new TinyColor(str.replaceAll(/deg|grad|rad|turn/g, "")).toRgbString();
}
/**
* 检查颜色是否有效
* @param {string} color - 待检查的颜色
* 如果颜色有效返回true，否则返回false
*/
function isValidColor(color) {
	if (!color) return false;
	return new TinyColor(color).isValid;
}
//#endregion
//#region src/color/generator.ts
function generatorColorVariables(colorItems) {
	const colorVariables = {};
	colorItems.forEach(({ alias, color, name }) => {
		if (color) {
			const colorsMap = getColors(new TinyColor(color).toHexString());
			let mainColor = colorsMap["500"];
			Object.keys(colorsMap).forEach((key) => {
				const colorValue = colorsMap[key];
				if (colorValue) {
					const hslColor = convertToHslCssVar(colorValue);
					colorVariables[`--${name}-${key}`] = hslColor;
					if (alias) colorVariables[`--${alias}-${key}`] = hslColor;
					if (key === "500") mainColor = hslColor;
				}
			});
			if (alias && mainColor) colorVariables[`--${alias}`] = mainColor;
		}
	});
	return colorVariables;
}
//#endregion
export { TinyColor, convertToHsl, convertToHslCssVar, convertToRgb, generatorColorVariables, isDarkColor, isLightColor, isValidColor };
