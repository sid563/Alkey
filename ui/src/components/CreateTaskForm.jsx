import { useState } from 'react';
import { createTask } from '../api/tasks';
import { useAuth } from '../context/AuthContext';
import { ErrorBanner, SuccessBanner } from './Feedback';

const EMPTY = { title: '', description: '', status: 'todo', priority: 'medium' };

/**
 * Used on both the admin and staff screens. For staff the API answers
 * 403, which is surfaced as a clear "blocked" message — demonstrating
 * role-based access from the UI.
 */
export default function CreateTaskForm({ onCreated }) {
  const { session } = useAuth();
  const [form, setForm] = useState(EMPTY);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const [success, setSuccess] = useState(null);

  const set = (field) => (e) => setForm({ ...form, [field]: e.target.value });

  const submit = async (e) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      const task = await createTask(session.token, form);
      setSuccess(`Task "${task.title}" created`);
      setForm(EMPTY);
      onCreated?.(task);
    } catch (err) {
      setError(
        err.status === 403
          ? 'You are not allowed to create tasks — only admins can. (403 Forbidden)'
          : err.message,
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="card" onSubmit={submit}>
      <h3>Create task</h3>
      <ErrorBanner message={error} />
      <SuccessBanner message={success} />
      <div className="row">
        <label>
          Title
          <input value={form.title} onChange={set('title')} required placeholder="Task title" />
        </label>
        <label>
          Status
          <select value={form.status} onChange={set('status')}>
            <option value="todo">todo</option>
            <option value="in_progress">in_progress</option>
            <option value="done">done</option>
          </select>
        </label>
        <label>
          Priority
          <select value={form.priority} onChange={set('priority')}>
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </select>
        </label>
      </div>
      <label>
        Description
        <input value={form.description} onChange={set('description')} placeholder="Optional description" />
      </label>
      <button type="submit" disabled={busy}>
        {busy ? 'Creating…' : 'Create task'}
      </button>
    </form>
  );
}
