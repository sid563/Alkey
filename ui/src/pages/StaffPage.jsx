import { useCallback, useEffect, useState } from 'react';
import { viewMyTasks } from '../api/tasks';
import CreateTaskForm from '../components/CreateTaskForm';
import { EmptyState, ErrorBanner, Spinner } from '../components/Feedback';
import TaskTable from '../components/TaskTable';
import { useAuth } from '../context/AuthContext';

export default function StaffPage() {
  const { session } = useAuth();
  const [data, setData] = useState(null);
  const [error, setError] = useState(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setData(await viewMyTasks(session.token));
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }, [session.token]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <>
      <div className="card">
        <div className="card-head">
          <h3>My assigned tasks</h3>
          <button onClick={refresh} disabled={busy}>
            {busy ? 'Refreshing…' : 'Refresh'}
          </button>
        </div>
        <ErrorBanner message={error} />
        {!data && !error ? (
          <Spinner label="Loading your tasks…" />
        ) : data ? (
          <>
            <p className="meta">
              <strong>{data.summary.total_assigned_tasks}</strong> task(s) assigned to{' '}
              {data.user.email} ·{' '}
              <span className={`badge ${data.cache.hit ? 'badge-hit' : 'badge-miss'}`}>
                cache {data.cache.hit ? 'HIT' : 'MISS'}
              </span>
            </p>
            {data.tasks.length === 0 ? (
              <EmptyState message="No tasks assigned to you yet." />
            ) : (
              <TaskTable tasks={data.tasks} />
            )}
          </>
        ) : null}
      </div>

      {/* Staff creating a task is forbidden; the form surfaces the 403 clearly. */}
      <CreateTaskForm />
    </>
  );
}
