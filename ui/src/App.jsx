import { AuthProvider, useAuth } from './context/AuthContext';
import AdminPage from './pages/AdminPage';
import LoginPage from './pages/LoginPage';
import StaffPage from './pages/StaffPage';

function Shell() {
  const { session, signOut } = useAuth();

  if (!session) return <LoginPage />;

  return (
    <div className="app">
      <header className="topbar">
        <h1>Task Manager</h1>
        <div className="userbox">
          <span>
            {session.user.full_name} · <span className="badge badge-role">{session.user.role}</span>
          </span>
          <button className="link" onClick={signOut}>
            Sign out
          </button>
        </div>
      </header>
      <main>{session.user.role === 'admin' ? <AdminPage /> : <StaffPage />}</main>
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <Shell />
    </AuthProvider>
  );
}
