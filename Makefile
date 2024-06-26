update:
	rm -rf Cargo.lock
	echo '⚙️  Updating crate...'
	cargo upgrade \
		-p holochain_types@$(shell jq .holochain_types ./version-manager.json) \
		-p holofuel_types@$(shell jq .holofuel_types ./version-manager.json) \
		-p holochain_keystore@$(shell jq .holochain_keystore ./version-manager.json) \
		-p lair_keystore_api@$(shell jq .lair_keystore_api ./version-manager.json) \
		-p holochain_conductor_api@$(shell jq .holochain_conductor_api ./version-manager.json) \
		-p holochain_websocket@$(shell jq .holochain_websocket ./version-manager.json) \
		-p mr_bundle@$(shell jq .mr_bundle ./version-manager.json)  --pinned