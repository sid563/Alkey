export function PriorityBadge({ priority }) {
  return <span className={`badge badge-${priority}`}>{priority}</span>;
}

/**
 * Task list table. When `selectable`, rows get checkboxes and
 * `selected`/`onToggle` drive the selection (used by the admin assign flow).
 */
export default function TaskTable({ tasks, selectable = false, selected, onToggle }) {
  return (
    <table className="task-table">
      <thead>
        <tr>
          {selectable && <th />}
          <th>Title</th>
          <th>Description</th>
          <th>Status</th>
          <th>Priority</th>
          <th>Assigned to</th>
        </tr>
      </thead>
      <tbody>
        {tasks.map((t) => (
          <tr key={t.id}>
            {selectable && (
              <td>
                <input
                  type="checkbox"
                  checked={selected.has(t.id)}
                  onChange={() => onToggle(t.id)}
                  aria-label={`Select ${t.title}`}
                />
              </td>
            )}
            <td>{t.title}</td>
            <td className="muted">{t.description}</td>
            <td>{t.status}</td>
            <td>
              <PriorityBadge priority={t.priority} />
            </td>
            <td>{t.assigned_to ?? <span className="muted">unassigned</span>}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
