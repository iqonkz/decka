export default function PageDots({ count, active }: { count: number; active: number }) {
  if (count <= 1) return null;
  return (
    <div className="page-dots">
      {Array.from({ length: count }, (_, i) => (
        <span key={i} className={i === active ? "dot active" : "dot"} />
      ))}
    </div>
  );
}
