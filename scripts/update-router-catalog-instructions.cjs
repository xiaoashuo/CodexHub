const fs = require('fs');
const path = require('path');

const home = process.env.USERPROFILE || process.env.HOME;
if (!home) {
  throw new Error('USERPROFILE/HOME is not set');
}

const configPath = path.join(home, '.codex', 'ai-router-workspace', 'config', 'router_provider_config.json');
const catalogPath = path.join(home, '.codex', 'ai-router-workspace', 'config', 'codex_router_catalog.json');
const cachePath = path.join(home, '.codex', 'models_cache.json');
const routerDescription = 'Custom model forwarded through local router.';
const instructionPrefix = 'You are Codex, a coding agent routed through the local AI Router.';

const routes = JSON.parse(fs.readFileSync(configPath, 'utf8'));

function updateCatalogFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return { filePath, updated: 0, missing: true };
  }

  const root = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  if (!Array.isArray(root.models)) {
    root.models = [];
  }

  const template = root.models[0] || {};
  let updated = 0;

  for (const [slug, route] of Object.entries(routes)) {
    if (route.enabled === false) {
      continue;
    }

    const displayName = String(route.displayName || slug).trim() || slug;
    const realModel = String(route.realModel || '').trim();
    let model = root.models.find((item) => item && (item.slug === slug || item.display_name === displayName));

    if (!model) {
      model = JSON.parse(JSON.stringify(template));
      root.models.push(model);
    }

    model.slug = slug;
    model.display_name = displayName;
    model.description = routerDescription;
    model.base_instructions = realModel
      ? `${instructionPrefix}\nThe active upstream model for this route is ${realModel}.`
      : instructionPrefix;
    model.priority = -10;
    model.availability_nux = null;
    model.visibility = 'list';
    model.supported_in_api = true;
    updated += 1;
  }

  fs.writeFileSync(filePath, `${JSON.stringify(root, null, 2)}\n`, 'utf8');
  return { filePath, updated };
}

const results = [updateCatalogFile(catalogPath), updateCatalogFile(cachePath)];
console.log(JSON.stringify(results, null, 2));
