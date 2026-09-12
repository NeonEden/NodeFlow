import { useState, useCallback, useRef } from 'react';
import { CustomNode, HistorySnapshot } from '../types';
import { Edge } from 'reactflow';

const MAX_HISTORY = 30;

export function useUndoRedo(initialNodes: CustomNode[], initialEdges: Edge[]) {
  const [past, setPast] = useState<HistorySnapshot[]>([]);
  const [future, setFuture] = useState<HistorySnapshot[]>([]);

  // Keep a ref to the latest state to avoid race conditions in callbacks
  const currentRef = useRef<HistorySnapshot>({
    nodes: initialNodes,
    edges: initialEdges,
  });

  const updateCurrent = useCallback((nodes: CustomNode[], edges: Edge[]) => {
    currentRef.current = { nodes, edges };
  }, []);

  const takeSnapshot = useCallback((nodes: CustomNode[], edges: Edge[]) => {
    setPast((prev) => {
      const next = [...prev, { nodes: currentRef.current.nodes, edges: currentRef.current.edges }];
      if (next.length > MAX_HISTORY) {
        return next.slice(next.length - MAX_HISTORY);
      }
      return next;
    });
    setFuture([]);
    currentRef.current = { nodes, edges };
  }, []);

  const undo = useCallback((): HistorySnapshot | null => {
    if (past.length === 0) return null;

    const previous = past[past.length - 1];
    const newPast = past.slice(0, past.length - 1);

    setFuture((prev) => [currentRef.current, ...prev]);
    setPast(newPast);
    currentRef.current = previous;

    return previous;
  }, [past]);

  const redo = useCallback((): HistorySnapshot | null => {
    if (future.length === 0) return null;

    const next = future[0];
    const newFuture = future.slice(1);

    setPast((prev) => [...prev, currentRef.current]);
    setFuture(newFuture);
    currentRef.current = next;

    return next;
  }, [future]);

  const resetHistory = useCallback((nodes: CustomNode[], edges: Edge[]) => {
    setPast([]);
    setFuture([]);
    currentRef.current = { nodes, edges };
  }, []);

  return {
    canUndo: past.length > 0,
    canRedo: future.length > 0,
    undoCount: past.length,
    redoCount: future.length,
    takeSnapshot,
    undo,
    redo,
    resetHistory,
    updateCurrent,
  };
}
