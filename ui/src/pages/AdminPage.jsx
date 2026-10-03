import { useCallback, useEffect, useState } from 'react';
import { assignTasks, listAllTasks } from '../api/tasks';
import CreateTaskForm from '../components/CreateTaskForm';
import { EmptyState, ErrorBanner, Spinner, SuccessBanner } from '../components/Feedback';
import TaskTable from '../components/TaskTable';
import { useAuth } from '../context/AuthContext';

export default function AdminPage() {
  const { session } = useAuth();
  const [tasks, setTasks] = useState(null);
  const [error, setError] = useState(null);
  const [selected, setSelected] = useState(new Set());
  const [assignee, setAssignee] = useState('jamesbond@example.com');
  const [assignBusy, setAssignBusy] = useState(false);
  const [assignError, setAssignError] = useState(null);
  const [assignSuccess, setAssignSuccess] = useState(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const res = await listAllTasks(session.token);
      setTasks(res.tasks);
    } catch (err) {
      setError(err.message);
      setTasks([]);
    }
  }, [session.token]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const toggle = (id) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  const submitAssign = async (e) => {
    e.preventDefault();
    setAssignBusy(true);
    setAssignError(null);
    setAssignSuccess(null);
    try {
      const res = await assignTasks(session.token, [...selected], assignee);
      setAssignSuccess(`Assigned ${res.assigned} task(s) to ${res.assignee_email}`);
      setSelected(new Set());
      await refresh();
    } catch (err) {
      setAssignError(err.message);
    } finally {
      setAssignBusy(false);
    }
  };

  return (
    <>
      <CreateTaskForm onCreated={refresh} />

      <div className="card">
        <h3>All tasks</h3>
        <ErrorBanner message={error} />
        {tasks === null ? (
          <Spinner label="Loading tasks…" />
        ) : tasks.length === 0 ? (
          <EmptyState message="No tasks yet. Create one above." />
        ) : (
          <>
            <TaskTable tasks={tasks} selectable selected={selected} onToggle={toggle} />
            <form className="assign-bar" onSubmit={submitAssign}>
              <label>
                Assign {selected.size} selected task(s) to
                <input
                  type="email"
                  value={assignee}
                  onChange={(e) => setAssignee(e.target.value)}
                  required
                />
              </label>
              <button type="submit" disabled={assignBusy || selected.size === 0}>
                {assignBusy ? 'Assigning…' : 'Assign'}
              </button>
            </form>
            <ErrorBanner message={assignError} />
            <SuccessBanner message={assignSuccess} />
          </>
        )}
      </div>
    </>
  );
}
