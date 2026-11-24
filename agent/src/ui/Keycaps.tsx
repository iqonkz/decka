import { t } from "@decka/shared";
import Button from "./Button";

export interface KeycapsProps {
  keys: string[];
  onChange?: () => void;
  changeLabel?: string;
  className?: string;
}

export default function Keycaps({
  keys,
  onChange,
  changeLabel,
  className,
}: KeycapsProps) {
  return (
    <div className={`keycaps-row${className ? ` ${className}` : ""}`}>
      <div className="keycaps-list">
        {keys.map((k, i) => (
          <kbd key={i} className="keycap">
            {k}
          </kbd>
        ))}
      </div>
      {onChange && (
        <Button variant="secondary" onClick={onChange}>
          {changeLabel ?? t("item.change")}
        </Button>
      )}
    </div>
  );
}
