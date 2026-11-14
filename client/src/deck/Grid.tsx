import type { ClientLayout, Page } from "@decka/shared";
import Tile from "./Tile";

export default function Grid({ page, grid }: { page: Page; grid: ClientLayout["grid"] }) {
  return (
    <div className="grid" style={{ gridTemplateColumns: `repeat(${grid.cols}, 1fr)`, gridTemplateRows: `repeat(${grid.rows}, 1fr)` }}>
      {page.items.map((item) => (
        <Tile key={item.id} item={item} />
      ))}
    </div>
  );
}
