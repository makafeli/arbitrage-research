import { useRef } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';

export type Tab<K extends string> = { key: K; label: string };

export function Tabs<K extends string>({ label, tabs, value, onChange }: { label: string; tabs: readonly Tab<K>[]; value: K; onChange: (key: K) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  function move(index: number, event: KeyboardEvent<HTMLButtonElement>) {
    event.preventDefault();
    onChange(tabs[index].key);
    refs.current[index]?.focus();
  }
  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    if (event.key === 'ArrowLeft') move((index - 1 + tabs.length) % tabs.length, event);
    else if (event.key === 'ArrowRight') move((index + 1) % tabs.length, event);
    else if (event.key === 'Home') move(0, event);
    else if (event.key === 'End') move(tabs.length - 1, event);
  }
  return <div className="tabs" role="tablist" aria-label={label}>{tabs.map((tab, index) => <button key={tab.key} ref={el => { refs.current[index] = el; }} role="tab" id={`tab-${tab.key}`} aria-selected={tab.key === value} aria-controls={`panel-${tab.key}`} tabIndex={tab.key === value ? 0 : -1} onClick={() => onChange(tab.key)} onKeyDown={event => onKeyDown(event, index)}>{tab.label}</button>)}</div>;
}

/** After a programmatic switch triggered from inside a panel that is about to hide, keep focus on the tab button instead of letting it fall to <body>. */
export function focusTab(key: string) { document.getElementById(`tab-${key}`)?.focus(); }

export function TabPanel<K extends string>({ tab, value, children }: { tab: K; value: K; children: ReactNode }) {
  return <div className="tabpanel" role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`} hidden={tab !== value}>{children}</div>;
}
