// expect: data/court-table, data/canlii-route, code/citation-function, code/lookup-key
type Level = { level: number };

const LEVELS: Record<string, Level> = {
  SCC: { level: 5 },
  ONCA: { level: 4 },
  BCCA: { level: 4 },
  ABCA: { level: 4 },
  ONSC: { level: 3 },
  BCSC: { level: 3 },
};

export const ROUTES = { FC: "ca/fct", HRTO: "on/onhrt", NBBR: "nb/NBQB" };

export function courtLevel(code: string): Level | null {
  return LEVELS[code] ?? null;
}

export function lookupLocal(args: { reporter: string }) {
  const reporter = args.reporter.toLowerCase().replace(/[^a-z0-9]/gu, "");
  return reporter;
}
