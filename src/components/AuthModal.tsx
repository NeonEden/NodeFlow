import React, { useState } from 'react';
import { UserProfile } from '../types';
import { DEMO_ACCOUNTS, registerUser } from '../services/auth';
import { X, UserCheck, LogOut, Sparkles, Shield, Mail, Lock, User, ArrowRight } from 'lucide-react';

interface AuthModalProps {
  currentUser: UserProfile | null;
  isOpen: boolean;
  onClose: () => void;
  onUserChange: (user: UserProfile | null) => void;
}

export const AuthModal: React.FC<AuthModalProps> = ({
  currentUser,
  isOpen,
  onClose,
  onUserChange,
}) => {
  const [tab, setTab] = useState<'profile' | 'login' | 'register'>('profile');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [name, setName] = useState('');
  const [role, setRole] = useState('Desarrollador & Creador');

  if (!isOpen) return null;

  const handleSelectDemo = (account: UserProfile) => {
    onUserChange(account);
    onClose();
  };

  const handleCustomLogin = (e: React.FormEvent) => {
    e.preventDefault();
    if (!email.trim()) return;

    const matched = DEMO_ACCOUNTS.find(
      (a) => a.email.toLowerCase() === email.trim().toLowerCase()
    );

    if (matched) {
      onUserChange(matched);
    } else {
      const newUser = registerUser(
        name || email.split('@')[0] || 'Usuario',
        email,
        role
      );
      onUserChange(newUser);
    }
    onClose();
  };

  const handleLogout = () => {
    onUserChange(null);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm animate-in fade-in duration-150">
      <div
        className="relative w-full max-w-md bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100 overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Glow backdrop decorative accent */}
        <div className="absolute -top-20 -right-20 w-40 h-40 bg-indigo-600/20 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -bottom-20 -left-20 w-40 h-40 bg-emerald-600/20 rounded-full blur-3xl pointer-events-none" />

        {/* Modal Header */}
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white shadow-md shadow-indigo-600/30">
              <Shield size={16} />
            </div>
            <div>
              <h2 className="text-base font-semibold text-slate-100">Autenticación NeuralMind</h2>
              <p className="text-xs text-slate-400">Gestión de sesión y mapas de usuario</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Navigation Tabs */}
        <div className="flex bg-slate-950 p-1 rounded-xl border border-slate-800/80 my-4 text-xs font-medium">
          <button
            type="button"
            onClick={() => setTab('profile')}
            className={`flex-1 py-1.5 rounded-lg transition-all ${
              tab === 'profile'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            {currentUser ? 'Cuenta Actual' : 'Seleccionar Usuario'}
          </button>
          <button
            type="button"
            onClick={() => setTab('login')}
            className={`flex-1 py-1.5 rounded-lg transition-all ${
              tab === 'login'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            Iniciar Sesión
          </button>
          <button
            type="button"
            onClick={() => setTab('register')}
            className={`flex-1 py-1.5 rounded-lg transition-all ${
              tab === 'register'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            Registrarse
          </button>
        </div>

        {/* TAB: Current Profile & Quick Demo Switch */}
        {tab === 'profile' && (
          <div className="space-y-4">
            {currentUser ? (
              <div className="p-4 rounded-xl bg-slate-950/70 border border-indigo-900/40 flex items-center gap-3.5">
                <img
                  src={currentUser.avatar}
                  alt={currentUser.name}
                  referrerPolicy="no-referrer"
                  className="w-12 h-12 rounded-full border-2 border-indigo-500 object-cover shadow"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold text-white truncate">{currentUser.name}</span>
                    <span className="text-[10px] bg-emerald-950 text-emerald-400 border border-emerald-800/50 px-1.5 py-0.2 rounded font-mono">
                      Activo
                    </span>
                  </div>
                  <p className="text-xs text-slate-400 truncate">{currentUser.email}</p>
                  <p className="text-[11px] text-indigo-400 mt-0.5">{currentUser.role}</p>
                </div>
              </div>
            ) : (
              <div className="p-3 bg-amber-950/30 border border-amber-800/40 rounded-xl text-xs text-amber-300">
                Actualmente estás en modo Invitado. Tus mapas se guardarán temporalmente en esta sesión.
              </div>
            )}

            <div>
              <label className="text-xs font-medium text-slate-400 mb-2 block flex items-center justify-between">
                <span>Cambio Rápido de Cuenta Demo:</span>
                <Sparkles size={13} className="text-indigo-400" />
              </label>
              <div className="space-y-2">
                {DEMO_ACCOUNTS.map((acc) => {
                  const isCurrent = currentUser?.id === acc.id;
                  return (
                    <button
                      key={acc.id}
                      type="button"
                      onClick={() => handleSelectDemo(acc)}
                      className={`w-full flex items-center justify-between p-2.5 rounded-xl border text-left transition-all ${
                        isCurrent
                          ? 'bg-indigo-950/40 border-indigo-500 ring-1 ring-indigo-500/40 text-white'
                          : 'bg-slate-950/60 border-slate-800/90 text-slate-300 hover:border-slate-700 hover:bg-slate-800/50'
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <img
                          src={acc.avatar}
                          alt={acc.name}
                          referrerPolicy="no-referrer"
                          className="w-9 h-9 rounded-full object-cover border border-slate-700"
                        />
                        <div>
                          <div className="text-xs font-semibold text-white">{acc.name}</div>
                          <div className="text-[10px] text-slate-400">{acc.role}</div>
                        </div>
                      </div>
                      {isCurrent ? (
                        <span className="flex items-center gap-1 text-[11px] text-indigo-400 font-medium">
                          <UserCheck size={14} /> Seleccionado
                        </span>
                      ) : (
                        <span className="text-[11px] text-slate-500 hover:text-slate-300">Conectar</span>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>

            {currentUser && (
              <button
                type="button"
                onClick={handleLogout}
                className="w-full flex items-center justify-center gap-2 py-2 px-4 rounded-xl border border-rose-900/50 bg-rose-950/20 text-rose-300 hover:bg-rose-900/30 text-xs font-medium transition-colors mt-3"
              >
                <LogOut size={14} /> Cerrar Sesión (Modo Invitado)
              </button>
            )}
          </div>
        )}

        {/* TAB: Sign In Form */}
        {tab === 'login' && (
          <form onSubmit={handleCustomLogin} className="space-y-3.5">
            <div>
              <label className="text-xs font-medium text-slate-300 block mb-1">Correo Electrónico</label>
              <div className="relative">
                <Mail size={14} className="absolute left-3 top-3 text-slate-400" />
                <input
                  type="email"
                  required
                  placeholder="ejemplo@neuralmind.ai"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="w-full pl-9 pr-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-xs text-white focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div>
              <label className="text-xs font-medium text-slate-300 block mb-1">Contraseña</label>
              <div className="relative">
                <Lock size={14} className="absolute left-3 top-3 text-slate-400" />
                <input
                  type="password"
                  placeholder="••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full pl-9 pr-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-xs text-white focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <button
              type="submit"
              className="w-full flex items-center justify-center gap-2 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-semibold shadow-lg shadow-indigo-600/30 transition-all active:scale-[0.98]"
            >
              Iniciar Sesión <ArrowRight size={14} />
            </button>
          </form>
        )}

        {/* TAB: Register Form */}
        {tab === 'register' && (
          <form onSubmit={handleCustomLogin} className="space-y-3.5">
            <div>
              <label className="text-xs font-medium text-slate-300 block mb-1">Nombre Completo</label>
              <div className="relative">
                <User size={14} className="absolute left-3 top-3 text-slate-400" />
                <input
                  type="text"
                  required
                  placeholder="Tu nombre o alias"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  className="w-full pl-9 pr-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-xs text-white focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div>
              <label className="text-xs font-medium text-slate-300 block mb-1">Correo Electrónico</label>
              <div className="relative">
                <Mail size={14} className="absolute left-3 top-3 text-slate-400" />
                <input
                  type="email"
                  required
                  placeholder="tu@correo.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="w-full pl-9 pr-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-xs text-white focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div>
              <label className="text-xs font-medium text-slate-300 block mb-1">Rol / Especialidad</label>
              <input
                type="text"
                placeholder="Ej. Estratega de Producto, Data Scientist"
                value={role}
                onChange={(e) => setRole(e.target.value)}
                className="w-full px-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-xs text-white focus:outline-none focus:border-indigo-500"
              />
            </div>

            <button
              type="submit"
              className="w-full flex items-center justify-center gap-2 py-2.5 bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl text-xs font-semibold shadow-lg shadow-emerald-600/30 transition-all active:scale-[0.98]"
            >
              Crear Cuenta y Entrar <ArrowRight size={14} />
            </button>
          </form>
        )}
      </div>
    </div>
  );
};
