/** Compare generated inline CSS without relaxing markup or application checks. */
export function matchesInlineCss(before, after) {
  if (before === after) return true;
  const pattern = /^(.*?\/\* TAILWIND_INLINE_START \*\/)([\s\S]*?)(\/\* TAILWIND_INLINE_END \*\/[\s\S]*)$/s;
  const left = before.match(pattern);
  const right = after.match(pattern);
  if (!left || !right || left[1] !== right[1] || left[3] !== right[3]) return false;

  // Native Lightning CSS builds differ in the final decimal of OKLab channels.
  // Keep the entire CSS structure and compare only corresponding color numbers.
  const colors = /oklab\([^)]*\)/g;
  const leftColors = [...left[2].matchAll(colors)];
  const rightColors = [...right[2].matchAll(colors)];
  if (leftColors.length !== rightColors.length) return false;
  if (left[2].replace(colors, "oklab()") !== right[2].replace(colors, "oklab()")) return false;
  const numbers = /-?(?:\d*\.\d+|\d+)(?:e[+-]?\d+)?/gi;
  return leftColors.every(([color], index) => {
    const other = rightColors[index][0];
    if (color === other) return true;
    if (color.replace(numbers, "#") !== other.replace(numbers, "#")) return false;
    const values = (color.match(numbers) || []).map(Number);
    const otherValues = (other.match(numbers) || []).map(Number);
    return values.length === otherValues.length &&
      values.every((value, i) => Math.abs(value - otherValues[i]) <= 1e-6);
  });
}
