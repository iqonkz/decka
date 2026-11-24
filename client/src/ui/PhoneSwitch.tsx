/** 44 x 26 pill toggle switch matching PC settings style (DESIGN.md 4.14, 4.16). */
export default function PhoneSwitch({
  checked,
  onChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={checked ? "phone-switch checked" : "phone-switch"}
      onClick={() => !disabled && onChange(!checked)}
    >
      <span className="phone-switch-knob" aria-hidden="true" />
    </button>
  );
}
