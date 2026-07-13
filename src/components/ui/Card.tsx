import type { ReactNode } from 'react';

type CardProps = {
  children: ReactNode;
  className?: string;
};

export function Card({ children, className = '' }: CardProps) {
  return <section className={`rounded-3xl border border-slate-200 bg-white shadow-sm ${className}`}>{children}</section>;
}

export function CardHeader({ children, className = '' }: CardProps) {
  return <div className={`border-b border-slate-100 px-6 py-5 ${className}`}>{children}</div>;
}

export function CardContent({ children, className = '' }: CardProps) {
  return <div className={`px-6 py-5 ${className}`}>{children}</div>;
}
