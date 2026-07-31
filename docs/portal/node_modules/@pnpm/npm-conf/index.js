'use strict';
const path = require('path');
const Conf = require('./lib/conf');
const _defaults = require('./lib/defaults');

// https://github.com/npm/cli/blob/latest/lib/config/core.js#L101-L200
module.exports = (opts, types, defaults) => {
	const conf = new Conf(Object.assign({}, _defaults.defaults, defaults), types);

	conf.add(Object.assign({}, opts), 'cli');
	const warnings = [];
	let failedToLoadBuiltInConfig = false;

	if (require.resolve.paths) {
		const paths = require.resolve.paths('npm');
		// Assume that last path in resolve paths is builtin modules directory
		let npmPath;
		try {
			npmPath = require.resolve('npm', {paths: paths.slice(-1)});
		} catch (error) {
			// Error will be thrown if module cannot be found.
			// Update the flag while loading builtin config failed.
			failedToLoadBuiltInConfig = true;
		}

		if (npmPath) {
			/**
			 *  According to https://github.com/npm/cli/blob/86f5bdb91f7a5971953a5171d32d6eeda6a2e972/lib/npm.js#L258
			 *  and https://github.com/npm/cli/blob/86f5bdb91f7a5971953a5171d32d6eeda6a2e972/lib/config/core.js#L92
			 */
			warnings.push(conf.addFile(path.resolve(path.dirname(npmPath), '..', 'npmrc'), 'builtin'));
		}
	}

	conf.addEnv();
	conf.loadPrefix();

	// SECURITY: The `project` and `workspace` .npmrc files added to the config
	// chain below are repository-controlled. They must NOT be able to choose
	// which files are loaded as the trusted `user` and `global` config sources.
	// `userconfig`, `globalconfig` and `prefix` decide those destinations, so a
	// project/workspace .npmrc could otherwise set e.g. `userconfig=.evil` and
	// have an attacker-supplied, repo-shipped file loaded as trusted `user`
	// config — which downstream consumers exempt from untrusted-env-expansion
	// filtering and which may set `tokenHelper` — leaking environment secrets.
	// Resolve those destinations now, from the trusted layers only (cli, env,
	// builtin, defaults), before the project/workspace layers are pushed.
	const trustedUserconfig = conf.get('userconfig');
	const trustedPrefix = conf.get('prefix');
	if (trustedPrefix) {
		const etc = path.resolve(trustedPrefix, 'etc');
		conf.root.globalconfig = path.resolve(etc, 'npmrc');
		conf.root.globalignorefile = path.resolve(etc, 'npmignore');
	}
	const trustedGlobalconfig = conf.get('globalconfig');

	const projectConf = path.resolve(conf.localPrefix, '.npmrc');
	const userConf = trustedUserconfig;

	if (!conf.get('global') && projectConf !== userConf) {
		warnings.push(conf.addFile(projectConf, 'project'));
	} else {
		conf.add({}, 'project');
	}

	// TODO: cover with tests that configs from workspace .npmrc have bigger priority
	// than the ones in userconfig
	if (conf.get('workspace-prefix') && conf.get('workspace-prefix') !== projectConf) {
		const workspaceConf = path.resolve(conf.get('workspace-prefix'), '.npmrc');
		warnings.push(conf.addFile(workspaceConf, 'workspace'));
	}

	warnings.push(conf.addFile(trustedUserconfig, 'user'));

	warnings.push(conf.addFile(trustedGlobalconfig, 'global'));
	conf.loadUser();

	const caFile = conf.get('cafile');

	if (caFile) {
		conf.loadCAFile(caFile);
	}

	return {
		config: conf,
		warnings: warnings.filter(Boolean),
		failedToLoadBuiltInConfig,
	};
};

Object.defineProperty(module.exports, 'defaults', {
	get() {
		return _defaults.defaults;
	},
	enumerable: true
})
