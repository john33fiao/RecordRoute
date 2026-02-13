import { Button } from './ui/button';

interface TaskActionControlsProps {
  canCancel?: boolean;
  canRetry?: boolean;
  retryLabel?: string;
  onCancel?: () => void;
  onRetry?: () => void;
  compact?: boolean;
}

export function TaskActionControls({ canCancel, canRetry, retryLabel = "재시도", onCancel, onRetry, compact = false }: TaskActionControlsProps) {
  if (!canCancel && !canRetry) return null;

  return (
    <div className={`flex items-center gap-2 ${compact ? '' : 'mt-2'}`}>
      {canCancel && onCancel && (
        <Button size="sm" variant="outline" onClick={onCancel} className="text-red-400 border-red-500/50 hover:bg-red-500/10">
          취소
        </Button>
      )}
      {canRetry && onRetry && (
        <Button size="sm" variant="outline" onClick={onRetry} className="text-amber-400 border-amber-500/50 hover:bg-amber-500/10">
          {retryLabel}
        </Button>
      )}
    </div>
  );
}
