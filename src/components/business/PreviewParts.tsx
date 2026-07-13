import { Card, CardContent } from '../ui/Card';

export function StatCard({ label, value, helper }: { label: string; value: string; helper: string }) {
  return (
    <Card>
      <CardContent>
        <div className="text-sm font-medium text-slate-500">{label}</div>
        <div className="mt-2 truncate text-xl font-bold text-slate-950">{value}</div>
        <div className="mt-2 truncate text-xs text-slate-400">{helper}</div>
      </CardContent>
    </Card>
  );
}

export function PathPreview({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="mb-1 font-semibold text-slate-700">{label}</div>
      <div className="break-all rounded-xl bg-slate-50 px-3 py-2 font-mono text-xs text-slate-500">{value}</div>
    </div>
  );
}

export function ReadOnlyInput({ label, value }: { label: string; value: string }) {
  return (
    <label className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">{label}</span>
      <input className="w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 outline-none" value={value} readOnly />
    </label>
  );
}
