//use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use sqlite::{Connection, State, Value};

/**
Get a token from the store.

# Arguments
* `tokens_file` - Location of the tokens file from the app configuration
* `name` - Name of the token you want

# Returns
- If the token was found, you get the token value
- If the token wasn't in the store you will get an empty string
- On failure, a sqlite::Error

# Examples
```
use redundinator::{tokens::{get_token, save_token}, testing::fixtures::Fixture};

let tokens_file = Fixture::blank("mytokens.db");
let name = "cloudprovider1_secret";
let value = "kjbgf240389gbh4398gh";
save_token(tokens_file.to_str(), name, value).unwrap();
let retrieved = get_token(tokens_file.to_str(), name).unwrap();
assert_eq!(value, retrieved);

let retrieved2 = get_token(tokens_file.to_str(), "non_existent_name").unwrap();
assert_eq!("", retrieved2);
```
*/
pub fn get_token(tokens_file: &str, name: &str) -> Result<String, sqlite::Error>
{
    let connection = connect(tokens_file)?;
    let select_query = "SELECT value FROM tokens WHERE name = :name LIMIT 1";
    let mut select_stmt = connection.prepare(select_query)?;
    select_stmt.bind((":name", name))?;
    if let State::Row = select_stmt.next()?
    {
        return select_stmt.read::<String, _>("value");
    }
    Ok("".to_string())
}

/**
Save a token to the store. Overwrite any existing token with that name.

# Arguments
* `tokens_file` - Location of the tokens file from the app configuration
* `name` - Name of the token
* `value` - Value of the token

# Returns
- On success, Ok(())
- On failure, a sqlite::Error

# Examples
```
use redundinator::{tokens::{get_token, save_token}, testing::fixtures::Fixture};

let tokens_file = Fixture::blank("mytokens.db");
let name = "cloudprovider1_secret";
let value = "kjbgf240389gbh4398gh";
save_token(tokens_file.to_str(), name, value).unwrap();
let retrieved = get_token(tokens_file.to_str(), name).unwrap();
assert_eq!(value, retrieved);
```
*/
pub fn save_token(tokens_file: &str, name: &str, value: &str) -> Result<(), sqlite::Error>
{
    let connection = connect(tokens_file)?;
    let save_query = "INSERT INTO tokens (name, value) VALUES (:name, :value)";
    let mut save_stmt = connection.prepare(save_query)?;
    save_stmt.bind::<&[(_, Value)]>(&[
        (":name", name.into()),
        (":value", value.into())
    ])?;
    while State::Row == save_stmt.next()? {}
    Ok(())
}

/**
Establish a sqlite connection to query the tokens file.

Also creates the file if it doesn't exist, and inside the file, creates the table if it doesn't exist.

# Arguments
* `tokens_file` - Location of the tokens file from the app configuration

# Returns
- On success, a Connection
- On failure, a sqlite::Error
*/
fn connect(tokens_file: &str) -> Result<Connection, sqlite::Error>
{
    let connection = sqlite::open(tokens_file)?;
    let query_create_table = "CREATE TABLE IF NOT EXISTS tokens (name TEXT PRIMARY KEY ON CONFLICT REPLACE, value TEXT)";
    connection.execute(query_create_table)?;
    Ok(connection)
}


