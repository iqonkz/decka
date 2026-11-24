import type { CSSProperties, ReactNode } from "react";
import PhIcon from "./PhIcon";

export interface PageHeaderProps {
  icon?: string;
  title: string;
  subtitle?: string;
  className?: string;
  style?: CSSProperties;
}

export function PageHeader({ icon, title, subtitle, className, style }: PageHeaderProps) {
  return (
    <header className={`settings-page-header${className ? ` ${className}` : ""}`} style={style}>
      {icon && (
        <div className="settings-page-header-badge">
          <PhIcon name={icon} size={22} color="var(--accent)" />
        </div>
      )}
      <div className="settings-page-header-text">
        <h2 className="settings-page-title">{title}</h2>
        {subtitle && <p className="settings-page-subtitle">{subtitle}</p>}
      </div>
    </header>
  );
}

export interface SectionHeaderProps {
  title: string;
  subtitle?: string;
  className?: string;
  style?: CSSProperties;
  id?: string;
}

export function SectionHeader({ title, subtitle, className, style, id }: SectionHeaderProps) {
  return (
    <div id={id} className={`settings-section-header${className ? ` ${className}` : ""}`} style={style}>
      <h3 className="settings-section-title">{title}</h3>
      {subtitle && <p className="settings-section-subtitle">{subtitle}</p>}
    </div>
  );
}

export interface GroupProps {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  id?: string;
}

export function Group({ children, className, style, id }: GroupProps) {
  return (
    <section id={id} className={`settings-group-card${className ? ` ${className}` : ""}`} style={style}>
      {children}
    </section>
  );
}

export interface RowProps {
  id?: string;
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
  htmlFor?: string;
}

export function Row({ id, title, description, children, className, style, htmlFor }: RowProps) {
  return (
    <div id={id} className={`settings-row${className ? ` ${className}` : ""}`} style={style}>
      <div className="settings-row-info">
        {htmlFor ? (
          <label htmlFor={htmlFor} className="settings-row-title">
            {title}
          </label>
        ) : (
          <span className="settings-row-title">{title}</span>
        )}
        {description && <div className="settings-row-desc">{description}</div>}
      </div>
      {children && <div className="settings-row-control">{children}</div>}
    </div>
  );
}

export function SettingsEmpty({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={`settings-empty${className ? ` ${className}` : ""}`}>{children}</div>;
}

export default {
  Group,
  Row,
  PageHeader,
  SectionHeader,
  SettingsEmpty,
};
