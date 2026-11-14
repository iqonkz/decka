import type { Page } from "@decka/shared";
import Tile from "./Tile";

export default function Grid({ page }: { page: Page }) {
  return (
    <div className="grid">
      {page.items.map((item) => (
        <Tile key={item.id} item={item} />
      ))}
    </div>
  );
}
