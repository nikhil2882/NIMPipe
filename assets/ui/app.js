let models = [];

async function api(path, opts = {}) {
    const res = await fetch(path, {
        headers: { 'Content-Type': 'application/json' },
        ...opts,
    });
    return res.json();
}

function showMessage(text, isError = false) {
    const el = document.getElementById('message');
    el.textContent = text;
    el.classList.remove('hidden', 'error');
    if (isError) el.classList.add('error');
    setTimeout(() => el.classList.add('hidden'), 5000);
}

async function loadStatus() {
    try {
        const data = await api('/health');
        const el = document.getElementById('proxy-status');
        el.textContent = 'Proxy Online';
        el.classList.add('ok');
        el.classList.remove('error');
    } catch (e) {
        const el = document.getElementById('proxy-status');
        el.textContent = 'Proxy Offline';
        el.classList.add('error');
        el.classList.remove('ok');
    }
}

async function loadModels() {
    const res = await api('/admin/models');
    if (!res.success) {
        showMessage('Failed to load models: ' + res.error, true);
        return;
    }
    models = res.data.models || [];
    renderModels();
    document.getElementById('registry-editor').value = tomlString(res.data);
}

function tomlString(file) {
    // Very basic TOML serializer for display/edit round-trip.
    const out = [];
    for (const m of file.models) {
        out.push('[[models]]');
        out.push(`openai_id = ${JSON.stringify(m.openai_id)}`);
        out.push(`backend_id = ${JSON.stringify(m.backend_id)}`);
        if (m.description) out.push(`description = ${JSON.stringify(m.description)}`);
        if (m.max_tokens_cap != null) out.push(`max_tokens_cap = ${m.max_tokens_cap}`);
        out.push(`supports_streaming = ${m.supports_streaming}`);
        out.push(`supports_tools = ${m.supports_tools}`);
        if (m.status_poll_path) out.push(`status_poll_path = ${JSON.stringify(m.status_poll_path)}`);
        if (m.default_params && Object.keys(m.default_params).length) {
            out.push('[models.default_params]');
            for (const [k, v] of Object.entries(m.default_params)) {
                out.push(`${k} = ${JSON.stringify(v)}`);
            }
        }
        if (m.injected_params && Object.keys(m.injected_params).length) {
            out.push('[models.injected_params]');
            for (const [k, v] of Object.entries(m.injected_params)) {
                out.push(`${k} = ${JSON.stringify(v)}`);
            }
        }
        if (m.strip_params && m.strip_params.length) {
            out.push(`strip_params = [${m.strip_params.map(s => JSON.stringify(s)).join(', ')}]`);
        }
        out.push('');
    }
    return out.join('\n');
}

function renderModels() {
    const grid = document.getElementById('models-grid');
    grid.innerHTML = '';
    for (const m of models) {
        const card = document.createElement('div');
        card.className = 'model-card';
        const thinking = Object.keys(m.injected_params || {}).some(k => k.includes('thinking'));
        const badges = [];
        if (m.supports_streaming) badges.push('<span class="badge">stream</span>');
        if (m.supports_tools) badges.push('<span class="badge">tools</span>');
        if (thinking) badges.push('<span class="badge warn">thinking</span>');
        card.innerHTML = `
            <h3>${escapeHtml(m.openai_id)}</h3>
            <div class="backend">${escapeHtml(m.backend_id)}</div>
            <div class="badges">${badges.join('')}</div>
            <div class="model-actions">
                <button class="btn small" onclick="testModel('${escapeHtml(m.openai_id)}')">Test</button>
            </div>
        `;
        grid.appendChild(card);
    }
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

async function saveRegistry() {
    const content = document.getElementById('registry-editor').value;
    const res = await api('/admin/models', {
        method: 'PUT',
        body: JSON.stringify({ content }),
    });
    if (res.success) {
        showMessage('Registry saved.');
        await loadModels();
    } else {
        showMessage('Save failed: ' + res.error, true);
    }
}

async function reloadConfig() {
    const res = await api('/admin/reload', { method: 'POST' });
    if (res.success) {
        showMessage('Config reloaded.');
        await loadModels();
    } else {
        showMessage('Reload failed: ' + res.error, true);
    }
}

async function testModel(openaiId) {
    document.getElementById('test-model-name').textContent = openaiId;
    document.getElementById('test-result').textContent = 'Running…';
    document.getElementById('test-modal').classList.remove('hidden');

    const res = await api('/admin/test', {
        method: 'POST',
        body: JSON.stringify({ openai_id: openaiId }),
    });

    if (res.success) {
        document.getElementById('test-result').textContent = JSON.stringify(JSON.parse(res.data.response), null, 2);
    } else {
        document.getElementById('test-result').textContent = 'Error: ' + res.error;
    }
}

async function testDefaultModel() {
    if (!models.length) return;
    await testModel(models[0].openai_id);
}

async function loadEvents() {
    const res = await api('/admin/events');
    if (!res.success) return;
    const list = document.getElementById('events-list');
    list.innerHTML = '';
    for (const ev of res.data) {
        const li = document.createElement('li');
        const levelClass = ev.level.toLowerCase() === 'error' ? 'level-error' : ev.level.toLowerCase() === 'warn' ? 'level-warn' : 'level-info';
        li.innerHTML = `<span class="time">${new Date(ev.timestamp).toLocaleTimeString()}</span><span class="${levelClass}">[${ev.level}]</span> ${escapeHtml(ev.message)}`;
        list.appendChild(li);
    }
}

document.getElementById('save-registry-btn').addEventListener('click', saveRegistry);
document.getElementById('reload-btn').addEventListener('click', reloadConfig);
document.getElementById('test-default-btn').addEventListener('click', testDefaultModel);
document.getElementById('close-modal-btn').addEventListener('click', () => {
    document.getElementById('test-modal').classList.add('hidden');
});

document.getElementById('add-model-btn').addEventListener('click', () => {
    const editor = document.getElementById('registry-editor');
    editor.value += '\n[[models]]\nopenai_id = "new-model"\nbackend_id = "vendor/model-id"\nsupports_streaming = true\nsupports_tools = true\n';
    editor.focus();
});

async function init() {
    await loadStatus();
    await loadModels();
    await loadEvents();
    setInterval(loadStatus, 5000);
    setInterval(loadEvents, 3000);
}

init();
