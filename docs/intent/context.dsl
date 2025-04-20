workspace {
	model {
		user = person "Client System" "A system that needs to lookup or store entity mappings"
		
		udex = softwareSystem "Udex" "A universal lookup directory for entities that is lightweight, fast and efficient for high transaction volumes" {
			server = container "Server" "Provides APIs and business logic, stateless and horizontally scalable" "Rust, gRPC/REST"
			datastore = container "Datastore" "Stores index state and handles transactions" "SQLite/Postgres"
			config = container "Configuration" "Determines datastore config and index configurations" "YAML"
		}
		
		user -> udex "Looks up and stores entity mappings using"
		user -> server "Makes API calls to" "gRPC/REST over TLS"
		server -> datastore "Reads from and writes to"
		server -> config "Reads configuration from"
	}
	
	views {
		systemContext udex "UdexContext" {
			include *
			autoLayout
			description "The system context diagram for the Udex universal lookup directory"
		}
		
		theme default
	}
}
