import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setLanguage } from "@decka/shared";
import { PageHeader, Row, SettingsEmpty } from "../src/ui/SettingsGroup";
import VoiceCommands from "../src/tabs/VoiceCommands";
import Sidebar from "../src/ui/Sidebar";

describe("T31.1b Settings shell & Phosphor icons", () => {
  it("renders PageHeader (24px icon), Row (18px icon), and uniform SettingsEmpty (40px icon)", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <div>
        <PageHeader icon="gear-six" title="Настройки" subtitle="Описание раздела" />
        <Row icon="bell" title="Уведомления" description="Подпись строки">
          <span>ctrl</span>
        </Row>
        <SettingsEmpty icon="tray" action={<button type="button">Добавить</button>}>
          Пустой список
        </SettingsEmpty>
      </div>,
    );
    expect(html).toContain("width:24px;height:24px");
    expect(html).toContain("width:18px;height:18px");
    expect(html).toContain("width:40px;height:40px");
    expect(html).toContain("settings-empty-text");
    expect(html).toContain("Добавить");
  });

  it("renders Sidebar with 20px Phosphor icons and VoiceCommands without lucide-react", () => {
    setLanguage("ru");
    const sbHtml = renderToStaticMarkup(
      <Sidebar activePage="voice" onNavigate={() => {}} />,
    );
    expect(sbHtml).toContain("width:20px;height:20px");
    expect(sbHtml).toContain("sidebar-group-title");

    const cmdHtml = renderToStaticMarkup(<VoiceCommands />);
    expect(cmdHtml).not.toContain("lucide-");
    expect(cmdHtml).toContain("ph-icon");
  });
});
