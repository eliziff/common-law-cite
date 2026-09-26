// expect: code/short-form-logic, code/pinpoint-format, regex/reporter
type Form = "full" | "supra" | "ibid";

export function displayed(previous: string | null, id: string, firstNote: number | undefined, name: string): string {
  const form: Form = previous === id ? "ibid" : firstNote ? "supra" : "full";
  if (form === "ibid") return "Ibid";
  return firstNote ? `${name}, supra note ${firstNote}` : name;
}

export function label(kind: string, plural: boolean, value: string) {
  return `${kind === "paragraph" ? plural ? "paras" : "para" : plural ? "ss" : "s"} ${value}`;
}

export const toEnglish = (citation: string) => citation.replace(/\bR\.?\s?C\.?\s?S\.?(?=\s+\d)/gu, "S.C.R.");
