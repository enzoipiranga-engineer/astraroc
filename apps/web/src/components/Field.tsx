import { useEffect, useState } from "react";
import { fromDisplay, toDisplay, unitLabel, type Unit } from "../units";

interface NumProps {
  label: string;
  value: number | null | undefined;
  unit: Unit;
  help?: string;
  optional?: boolean;
  placeholder?: string;
  onChange: (si: number | null) => void;
}

/** Campo numérico em unidade de exibição; aceita vírgula decimal e só grava ao sair. */
export function NumberField({ label, value, unit, help, optional, placeholder, onChange }: NumProps) {
  const shown = value === null || value === undefined ? "" : String(+toDisplay(value, unit).toPrecision(6));
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);
  const commit = () => {
    const t = text.trim().replace(",", ".");
    if (t === "" && optional) return onChange(null);
    const v = Number(t);
    if (Number.isFinite(v)) onChange(fromDisplay(v, unit));
    else setText(shown);
  };
  return (
    <label className="field" title={help}>
      <span>
        {label}
        {unitLabel[unit] && <em> ({unitLabel[unit]})</em>}
      </span>
      <input
        inputMode="decimal"
        value={text}
        placeholder={placeholder ?? (optional ? "calculado" : "")}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
    </label>
  );
}

interface SelectProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string; group?: string }[];
  help?: string;
  onChange: (v: T) => void;
}

export function SelectField<T extends string>({ label, value, options, help, onChange }: SelectProps<T>) {
  const groups = [...new Set(options.map((o) => o.group ?? ""))];
  return (
    <label className="field" title={help}>
      <span>{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value as T)}>
        {groups.map((g) =>
          g ? (
            <optgroup key={g} label={g}>
              {options.filter((o) => o.group === g).map((o) => (
                <option key={o.value} value={o.value}>{o.label}</option>
              ))}
            </optgroup>
          ) : (
            options.filter((o) => !o.group).map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))
          ),
        )}
      </select>
    </label>
  );
}

export function TextField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  return (
    <label className="field">
      <span>{label}</span>
      <input value={text} onChange={(e) => setText(e.target.value)} onBlur={() => text !== value && onChange(text)} />
    </label>
  );
}

export function CheckField({ label, value, help, onChange }: { label: string; value: boolean; help?: string; onChange: (v: boolean) => void }) {
  return (
    <label className="field check" title={help}>
      <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}
