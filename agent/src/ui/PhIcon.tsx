/** A Phosphor icon (regular, 20 px by default) from the copy of the library that ships with the window; follows the text colour. */
export default function PhIcon({ name, size = 20 }: { name: string; size?: number }) {
  const mask = `url("phosphor/regular/${name}.svg")`;
  return (
    <span
      aria-hidden="true"
      className="ph-icon"
      style={{
        display: "inline-block",
        flex: "none",
        width: size,
        height: size,
        backgroundColor: "currentColor",
        WebkitMaskImage: mask,
        maskImage: mask,
        WebkitMaskSize: "contain",
        maskSize: "contain",
        WebkitMaskRepeat: "no-repeat",
        maskRepeat: "no-repeat",
        WebkitMaskPosition: "center",
        maskPosition: "center",
      }}
    />
  );
}
