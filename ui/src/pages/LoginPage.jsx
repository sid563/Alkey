import { useState } from 'react';
import { latestEmailLog, login, verify2fa } from '../api/auth';
import { ErrorBanner, SuccessBanner } from '../components/Feedback';
import { useAuth } from '../context/AuthContext';

export default function LoginPage() {
  const { signIn } = useAuth();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [code, setCode] = useState('');
  const [challengeId, setChallengeId] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const [info, setInfo] = useState(null);

  const submitCredentials = async (e) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await login(email, password);
      setChallengeId(res.login_challenge_id);
      setInfo('A verification code was sent to your email.');
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  };

  const submitCode = async (e) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await verify2fa(challengeId, code.trim());
      signIn(res.access_token, res.user);
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  };

  // Dev convenience: pull the code straight from the local email log.
  const fetchDevCode = async () => {
    setError(null);
    try {
      const log = await latestEmailLog(email);
      const match = log.body.match(/\d{6}/);
      if (match) setCode(match[0]);
      else setError('No code found in the latest email.');
    } catch (err) {
      setError(err.message);
    }
  };

  return (
    <div className="login-wrap">
      <div className="card login-card">
        <h2>Task Manager</h2>
        <ErrorBanner message={error} />
        {!challengeId ? (
          <form onSubmit={submitCredentials}>
            <label>
              Email
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="admin@example.com"
                required
                autoFocus
              />
            </label>
            <label>
              Password
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
              />
            </label>
            <button type="submit" disabled={busy}>
              {busy ? 'Signing in…' : 'Sign in'}
            </button>
          </form>
        ) : (
          <form onSubmit={submitCode}>
            <SuccessBanner message={info} />
            <label>
              6-digit verification code
              <input
                value={code}
                onChange={(e) => setCode(e.target.value)}
                placeholder="123456"
                inputMode="numeric"
                pattern="\d{6}"
                required
                autoFocus
              />
            </label>
            <button type="submit" disabled={busy}>
              {busy ? 'Verifying…' : 'Verify code'}
            </button>
            <button type="button" className="link" onClick={fetchDevCode}>
              Fetch code from dev inbox
            </button>
            <button
              type="button"
              className="link"
              onClick={() => {
                setChallengeId(null);
                setInfo(null);
                setCode('');
              }}
            >
              ← Back to login
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
