import React, { useState } from 'react';
import { Copy, Check, Play, Shield, Terminal, Plus, Trash2 } from 'lucide-react';
import type {
  Language,
  McpAuditEntry,
  McpClientConfigSnippets,
  McpGovernancePolicy,
  PrivacyRule,
} from '../types';

interface McpAndPrivacyViewProps {
  lang: Language;
  policy: McpGovernancePolicy;
  snippets: McpClientConfigSnippets;
  auditLogs: McpAuditEntry[];
  privacyRules: PrivacyRule[];
  onSavePolicy: (policy: McpGovernancePolicy) => Promise<void>;
  onInvokeInspector: (
    toolName: string,
    args: Record<string, unknown>,
  ) => Promise<{ result: unknown; audit_logs: McpAuditEntry[] }>;
  onTogglePrivacyRule: (id: string, isEnabled: boolean) => Promise<void>;
  onAddPrivacyRule: (kind: string, pattern: string, description: string) => Promise<void>;
  onDeletePrivacyRule: (id: string) => Promise<void>;
}

const MCP_TOOL_PRESETS: Record<string, Record<string, unknown>> = {
  get_live_context: { lookback_minutes: 90 },
  search_memory: { query: 'ScreenCaptureKit dHash MCP', limit: 5 },
  list_meetings: { limit: 5 },
  get_meeting_context: { meeting_id: 'mtg_arch_sync' },
  get_meeting_transcript: { meeting_id: 'mtg_arch_sync' },
  get_meeting_screenshots: { meeting_id: 'mtg_arch_sync' },
  get_person_dossier: { name: 'Markus' },
  get_project_context: { project: 'backtext' },
  get_user_profile: {},
  remember_agent_insight: {
    title: 'MCP Latenz-Check bestanden',
    content: 'Agent hat get_live_context in 8ms erfolgreich verifiziert.',
    project: 'backtext',
  },
};

export const McpAndPrivacyView: React.FC<McpAndPrivacyViewProps> = ({
  lang,
  policy,
  snippets,
  auditLogs,
  privacyRules,
  onSavePolicy,
  onInvokeInspector,
  onTogglePrivacyRule,
  onAddPrivacyRule,
  onDeletePrivacyRule,
}) => {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [selectedTool, setSelectedTool] = useState<string>('get_live_context');
  const [argsJson, setArgsJson] = useState<string>(
    JSON.stringify(MCP_TOOL_PRESETS.get_live_context, null, 2),
  );
  const [inspectorOutput, setInspectorOutput] = useState<string | null>(null);
  const [ruleKind, setRuleKind] = useState('app_bundle');
  const [rulePattern, setRulePattern] = useState('');
  const [ruleDesc, setRuleDesc] = useState('');

  const copyText = (key: string, val: string) => {
    navigator.clipboard?.writeText(val).catch(() => {});
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 1600);
  };

  const runTool = async () => {
    try {
      const parsed = JSON.parse(argsJson || '{}') as Record<string, unknown>;
      const res = await onInvokeInspector(selectedTool, parsed);
      const resultObj = res.result as { content?: Array<{ text?: string }> };
      const txt = resultObj?.content?.[0]?.text ?? JSON.stringify(res.result, null, 2);
      setInspectorOutput(txt);
    } catch (err) {
      setInspectorOutput(String(err));
    }
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de'
              ? 'MCP-Server, Agenten-Governance & Datenschutz'
              : 'MCP Server, Agent Governance & Privacy Shield'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Verbinde Claude Desktop, Cursor oder eigene KI-Agenten lokal über stdio & Streamable HTTP — mit voller Berechtigungskontrolle und Audit-Log.'
              : 'Connect Claude Desktop, Cursor, or custom AI agents over stdio & Streamable HTTP with granular governance and audit logging.'}
          </p>
        </div>
      </header>

      <div className="two-col-grid">
        {/* 1-Click MCP Client Configurations */}
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <h2 className="section-heading">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
              <Terminal size={15} />
              {lang === 'de' ? '1-Klick Agenten-Anbindung' : '1-Click Agent Connection'}
            </span>
            <span className="pill pill-sage">stdio + HTTP :{policy.http_port}</span>
          </h2>

          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 5 }}>
              <strong style={{ fontSize: 12.5 }}>Claude Desktop (`claude_desktop_config.json`)</strong>
              <button
                type="button"
                className="mini-btn"
                onClick={() => copyText('claude', snippets.claude_desktop_json)}
              >
                {copiedKey === 'claude' ? <Check size={11} /> : <Copy size={11} />}{' '}
                {copiedKey === 'claude' ? 'Kopiert' : 'Kopieren'}
              </button>
            </div>
            <pre className="code-block">{snippets.claude_desktop_json}</pre>
          </div>

          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 5 }}>
              <strong style={{ fontSize: 12.5 }}>Cursor / Windsurf (`.cursor/mcp.json`)</strong>
              <button
                type="button"
                className="mini-btn"
                onClick={() => copyText('cursor', snippets.cursor_mcp_json)}
              >
                {copiedKey === 'cursor' ? <Check size={11} /> : <Copy size={11} />}{' '}
                {copiedKey === 'cursor' ? 'Kopiert' : 'Kopieren'}
              </button>
            </div>
            <pre className="code-block">{snippets.cursor_mcp_json}</pre>
          </div>

          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 5 }}>
              <strong style={{ fontSize: 12.5 }}>Claude Code CLI</strong>
              <button
                type="button"
                className="mini-btn"
                onClick={() => copyText('cli', snippets.claude_code_cli)}
              >
                {copiedKey === 'cli' ? <Check size={11} /> : <Copy size={11} />}{' '}
                {copiedKey === 'cli' ? 'Kopiert' : 'Kopieren'}
              </button>
            </div>
            <pre className="code-block">{snippets.claude_code_cli}</pre>
          </div>
        </section>

        {/* Granular MCP Governance Switches */}
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
          <h2 className="section-heading">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
              <Shield size={15} />
              {lang === 'de' ? 'MCP-Zugriffsrichtlinien (Governance)' : 'MCP Governance Policy'}
            </span>
          </h2>

          {(
            [
              ['enabled', 'MCP-Server aktivieren (stdio & localhost HTTP)'],
              ['allow_live_context', 'Live-Arbeitskontext freigeben (get_live_context)'],
              ['allow_raw_transcripts', 'Wortgetreue Volltranskripte freigeben (get_meeting_transcript)'],
              ['allow_screenshare_ocr', 'Screen-Sharing-Folien & OCR freigeben (get_meeting_screenshots)'],
              ['allow_clipboard', 'Gefilterte Zwischenablage in Live-Kontext einbeziehen'],
              ['allow_agent_writeback', 'Agenten-Erkenntnisse zurückspeichern erlauben (remember_agent_insight)'],
            ] as const
          ).map(([field, label]) => (
            <label
              key={field}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '9px 11px',
                borderRadius: 6,
                background: 'var(--bg-surface-subtle)',
                cursor: 'pointer',
              }}
            >
              <span style={{ fontSize: 13 }}>{label}</span>
              <input
                type="checkbox"
                checked={Boolean(policy[field])}
                onChange={(e) => onSavePolicy({ ...policy, [field]: e.target.checked })}
              />
            </label>
          ))}
        </section>
      </div>

      {/* Live MCP Tool Inspector + Real-time Audit Log */}
      <section className="surface-sheet">
        <h2 className="section-heading">
          <span>{lang === 'de' ? 'Live MCP Tool-Inspektor (Echter Rust JSON-RPC Call)' : 'Live MCP Tool Inspector'}</span>
          <span className="mono-ts">10 Tools aktiv</span>
        </h2>

        <div className="two-col-grid">
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              MCP Tool wählen
              <select
                id="mcp-inspector-tool-select"
                className="select-field"
                style={{ marginTop: 4 }}
                value={selectedTool}
                onChange={(e) => {
                  const next = e.target.value;
                  setSelectedTool(next);
                  setArgsJson(JSON.stringify(MCP_TOOL_PRESETS[next] ?? {}, null, 2));
                }}
              >
                {Object.keys(MCP_TOOL_PRESETS).map((tool) => (
                  <option key={tool} value={tool}>
                    {tool}
                  </option>
                ))}
              </select>
            </label>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              JSON-Argumente
              <textarea
                className="textarea-field"
                rows={5}
                style={{ marginTop: 4, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                value={argsJson}
                onChange={(e) => setArgsJson(e.target.value)}
              />
            </label>

            <button
              type="button"
              id="run-mcp-inspector-btn"
              className="btn btn-primary"
              style={{ alignSelf: 'flex-start' }}
              onClick={runTool}
            >
              <Play size={14} />
              Tool gegen lokalen MCP-Server ausführen
            </button>
          </div>

          <div>
            <div style={{ fontSize: 12.5, fontWeight: 600, marginBottom: 6 }}>
              Antwort an KI-Agent:
            </div>
            <pre className="code-block" style={{ minHeight: 190, maxHeight: 290, overflowY: 'auto' }}>
              {inspectorOutput ??
                (lang === 'de'
                  ? 'Wähle links ein MCP-Tool (z. B. get_live_context oder search_memory) und klicke auf Ausführen.'
                  : 'Select an MCP tool on the left and click Run.')}
            </pre>
          </div>
        </div>

        <div style={{ marginTop: 20 }}>
          <div className="section-heading">
            <span>{lang === 'de' ? 'Echtzeit MCP-Audit-Log' : 'Real-Time MCP Audit Log'}</span>
            <span className="mono-ts">{auditLogs.length} Einträge</span>
          </div>
          <table className="data-table">
            <thead>
              <tr>
                <th>Zeit</th>
                <th>Client</th>
                <th>Transport</th>
                <th>Tool</th>
                <th>Abfrage</th>
                <th>Latenz</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {auditLogs.slice(0, 10).map((log) => (
                <tr key={log.id}>
                  <td className="mono-ts">
                    {new Date(log.created_at).toLocaleTimeString([], {
                      hour: '2-digit',
                      minute: '2-digit',
                      second: '2-digit',
                    })}
                  </td>
                  <td>{log.client_name}</td>
                  <td className="mono-ts">{log.transport}</td>
                  <td className="mono-ts">{log.tool_name}</td>
                  <td>{log.query_summary}</td>
                  <td className="mono-ts">{log.duration_ms} ms</td>
                  <td>
                    <span className={`pill ${log.allowed ? 'pill-sage' : 'pill-terracotta'}`}>
                      {log.allowed ? 'Erlaubt' : 'Blockiert'}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Privacy Blocklist Rules */}
      <section className="surface-sheet">
        <h2 className="section-heading">
          <span>
            {lang === 'de'
              ? 'Privacy Shield: Ausgeschlossene Apps, Domains & Fenster'
              : 'Privacy Shield: Excluded Apps, Domains & Windows'}
          </span>
        </h2>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 14 }}>
          {privacyRules.map((rule) => (
            <div key={rule.id} className="transcript-row" style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
              <div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <span className="pill pill-neutral">{rule.kind}</span>
                  <code style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}>{rule.pattern}</code>
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary)', marginTop: 3 }}>
                  {rule.description}
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <input
                  type="checkbox"
                  checked={rule.is_enabled}
                  onChange={(e) => onTogglePrivacyRule(rule.id, e.target.checked)}
                />
                {!rule.is_builtin && (
                  <button
                    type="button"
                    className="mini-btn"
                    onClick={() => onDeletePrivacyRule(rule.id)}
                  >
                    <Trash2 size={12} />
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>

        <form
          onSubmit={async (e) => {
            e.preventDefault();
            if (!rulePattern.trim()) return;
            await onAddPrivacyRule(ruleKind, rulePattern, ruleDesc || 'Eigene Ausschlussregel');
            setRulePattern('');
            setRuleDesc('');
          }}
          style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}
        >
          <select
            className="select-field"
            style={{ width: 170 }}
            value={ruleKind}
            onChange={(e) => setRuleKind(e.target.value)}
          >
            <option value="app_bundle">App / Bundle-ID</option>
            <option value="domain_pattern">Domain / URL</option>
            <option value="window_title">Fenstertitel</option>
          </select>
          <input
            className="input-field"
            style={{ flex: 1, minWidth: 200 }}
            value={rulePattern}
            onChange={(e) => setRulePattern(e.target.value)}
            placeholder="z. B. com.apple.Terminal oder *internal-admin*"
          />
          <input
            className="input-field"
            style={{ flex: 1, minWidth: 200 }}
            value={ruleDesc}
            onChange={(e) => setRuleDesc(e.target.value)}
            placeholder="Beschreibung"
          />
          <button type="submit" className="btn">
            <Plus size={14} />
            Regel hinzufügen
          </button>
        </form>
      </section>
    </div>
  );
};
