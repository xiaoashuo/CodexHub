import { useState } from 'react';
import { Button } from '../ui/Button';
import type { ConfirmDialogState } from '../../types';

const confirmButtonVariantByDialogVariant = {
  warning: 'primary',
  danger: 'danger',
  info: 'primary',
} as const;

export function ConfirmDialog({ state, handleClose }: { state: ConfirmDialogState; handleClose: () => void }) {
  const [confirming, setConfirming] = useState(false);

  const handleConfirm = async () => {
    if (confirming) {
      return;
    }

    setConfirming(true);

    try {
      await state.onConfirm();
      handleClose();
    } finally {
      setConfirming(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/50 px-6 backdrop-blur-sm">
      <div className="w-full max-w-xl rounded-3xl bg-white shadow-2xl shadow-slate-950/20">
        <div className="border-b border-slate-100 px-6 py-5">
          <h3 className="text-xl font-bold text-slate-950">{state.title}</h3>
          <p className="mt-2 text-sm leading-6 text-slate-600">{state.description}</p>
          {state.detail && <pre className="mt-4 max-h-40 overflow-auto whitespace-pre-wrap rounded-2xl bg-slate-950 p-4 text-xs leading-5 text-slate-100">{state.detail}</pre>}
        </div>
        <div className="flex justify-end gap-3 bg-slate-50 px-6 py-5">
          <Button variant="secondary" onClick={handleClose} disabled={confirming}>{state.cancelText}</Button>
          <Button variant={confirmButtonVariantByDialogVariant[state.variant]} onClick={handleConfirm} disabled={confirming}>{confirming ? '处理中...' : state.confirmText}</Button>
        </div>
      </div>
    </div>
  );
}
