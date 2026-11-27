export default function PageDots({
  count,
  active,
  onSelect,
}: {
  count: number;
  active: number;
  onSelect?: (index: number) => void;
}) {
  if (count <= 1) return null;
  return (
    <div className="page-dots" aria-hidden={onSelect ? undefined : "true"}>
      {Array.from({ length: count }, (_, i) => {
        const cls = i === active ? "dot active" : "dot";
        if (!onSelect) return <span key={i} className={cls} />;
        return (
          <button
            key={i}
            type="button"
            className="page-dot-btn"
            aria-label={`${i + 1} / ${count}`}
            aria-current={i === active ? "page" : undefined}
            onClick={() => onSelect(i)}
          >
            <span className={cls} />
          </button>
        );
      })}
    </div>
  );
}
