type BadgeTone = 'green' | 'blue' | 'amber' | 'slate' | 'rose';

type BadgeProps = {
  children: string;
  tone?: BadgeTone;
};

const toneClassName: Record<BadgeTone, string> = {
  green: 'bg-emerald-50 text-emerald-700 ring-emerald-600/20',
  blue: 'bg-blue-50 text-blue-700 ring-blue-600/20',
  amber: 'bg-amber-50 text-amber-700 ring-amber-600/20',
  slate: 'bg-slate-100 text-slate-700 ring-slate-600/20',
  rose: 'bg-rose-50 text-rose-700 ring-rose-600/20',
};

export function Badge({ children, tone = 'slate' }: BadgeProps) {
  return <span className={`inline-flex whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-semibold ring-1 ${toneClassName[tone]}`}>{children}</span>;
}
