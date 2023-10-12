use std::collections::HashMap;
use anyhow::Error;
use std::sync::Arc;
use std::process::Command;

struct Config{
    namespaces: HashMap<String,Box<Arc<Namespace>>>,
    links: HashMap<String,Box<Arc<Link>>>,
    interfaces: HashMap<String,Box<Arc<Interface>>>,
}


impl Config{
    fn new() -> Config {
        Config{
            namespaces: HashMap::new(),
            links: HashMap::new(),
            interfaces: HashMap::new(),
        }
    }
}

struct Link{
    name: String,
    subnet: String,
}

impl Link {
    fn new(name: String, subnet: String, config: &mut Config) -> anyhow::Result<Arc<Link>> {
        if let Some(r) = config.links.get(&name){
            return Err(anyhow::anyhow!("RouterLink {} already exists", r.name));
        }
        let r = Arc::new(Link{
            name: name.clone(),
            subnet,
        });
        config.links.insert(name, Box::new(r.clone()));
        Ok(r.clone())
    }
    fn attach(&self, ns1: Arc<Namespace>, ns2: Arc<Namespace>, config: &mut Config) -> anyhow::Result<(Arc<Interface>,Arc<Interface>)>{
        let name1 = format!("{}_{}", ns1.name.clone(), self.name);
        let name2 = format!("{}_{}", ns2.name.clone(), self.name);
        let veth = Veth{
            name: name1.clone(),
            peer: name2.clone(),
        };
        veth.create()?;


        let sn: ipnet::IpNet = self.subnet.parse()?;
        let pl = sn.prefix_len();
        let sn_v4: std::net::Ipv4Addr = sn.addr().to_string().parse()?;
        let sn_v4_octets = u32::from_be_bytes(sn_v4.octets());
        let ip1 = sn_v4_octets + 1;
        let ip2 = sn_v4_octets + 2;
        let ip1 = format!("{}/{}", std::net::Ipv4Addr::from(ip1.to_be_bytes()).to_string(), pl);
        let ip2 = format!("{}/{}", std::net::Ipv4Addr::from(ip2.to_be_bytes()).to_string(), pl);
        let i1 = Interface::new(name1.clone(), Some(ns1.clone()), Some(ip1.clone()), Some(3000), config)?;
        let i2 = Interface::new(name2.clone(), Some(ns2.clone()), Some(ip2.clone()), Some(3000), config)?;

        Ok((i1,i2))
    }
    
}


struct Route{
    dst: String,
    gateway: Vec<Arc<Interface>>,
}

struct Interface{
    name: String,
    ip: Option<String>,
    namespace: Option<Arc<Namespace>>,
    mtu: Option<u32>,
}

impl Interface {
    fn new(name: String, namespace: Option<Arc<Namespace>>, ip: Option<String>, mtu: Option<u32>, config: &mut Config) -> anyhow::Result<Arc<Interface>> {
        if let Some(r) = config.interfaces.get(&name){
            return Err(anyhow::anyhow!("Interface {} already exists", r.name));
        }
        let mut i = Interface{
            name: name.clone(),
            ip,
            namespace,
            mtu,
        };
        if let Some(namespace) = i.namespace.clone(){
            i.attach(namespace)?;
        }
        if let Some(ip) = i.ip.clone(){
            i.set_ip(ip)?;
        }
        if let Some(mtu) = i.mtu.clone(){
            i.set_mtu(mtu)?;
        }
        i.set_up()?;
        let r = Arc::new(i);
        config.interfaces.insert(name, Box::new(r.clone()));
        Ok(r.clone())
    }
    fn attach(&self, namespace: Arc<Namespace>) -> anyhow::Result<()>{
            let output = Command::new("ip")
            .arg("link")
            .arg("set")
            .arg(self.name.as_str())
            .arg("netns")
            .arg(namespace.name.as_str())
            .output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to attach interface to namespace: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }
    fn set_ip(&mut self, ip: String) -> anyhow::Result<()>{
        match &self.namespace{
            Some(namespace) => {
                let output = Command::new("ip")
                    .arg("netns")
                    .arg("exec")
                    .arg(namespace.name.as_str())
                    .arg("ip")
                    .arg("addr")
                    .arg("add")
                    .arg(ip.as_str())
                    .arg("dev")
                    .arg(self.name.as_str())
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set ip: {}", String::from_utf8_lossy(&output.stderr)));
                }
            },
            None => {
                let output = Command::new("ip")
                    .arg("addr")
                    .arg("add")
                    .arg(ip.as_str())
                    .arg("dev")
                    .arg(self.name.as_str())
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set ip: {}", String::from_utf8_lossy(&output.stderr)));
                }
            }
        }

        self.ip = Some(ip);
        Ok(())
    }
    fn set_mtu(&mut self, mtu: u32) -> anyhow::Result<()>{
        match &self.namespace{
            Some(namespace) => {
                let output = Command::new("ip")
                    .arg("netns")
                    .arg("exec")
                    .arg(namespace.name.as_str())
                    .arg("ip")
                    .arg("link")
                    .arg("set")
                    .arg("dev")
                    .arg(self.name.as_str())
                    .arg("mtu")
                    .arg(mtu.to_string().as_str())
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set mtu: {}", String::from_utf8_lossy(&output.stderr)));
                }
            },
            None => {
                let output = Command::new("ip")
                    .arg("link")
                    .arg("set")
                    .arg("dev")
                    .arg(self.name.as_str())
                    .arg("mtu")
                    .arg(mtu.to_string().as_str())
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set mtu: {}", String::from_utf8_lossy(&output.stderr)));
                }
            }   
        }
        self.mtu = Some(mtu);
        Ok(())
    }
    fn set_up(&mut self) -> anyhow::Result<()>{
        match &self.namespace{
            Some(namespace) => {
                let output = Command::new("ip")
                    .arg("netns")
                    .arg("exec")
                    .arg(namespace.name.as_str())
                    .arg("ip")
                    .arg("link")
                    .arg("set")
                    .arg("dev")
                    .arg(self.name.as_str())
                    .arg("up")
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set up: {}", String::from_utf8_lossy(&output.stderr)));
                }
            },
            None => {
                let output = Command::new("ip")
                    .arg("link")
                    .arg("set")
                    .arg("dev")
                    .arg(self.name.as_str())
                    .arg("up")
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!("Failed to set up: {}", String::from_utf8_lossy(&output.stderr)));
                }
            }
        }
        Ok(())
    }
}



struct Veth{
    name: String,
    peer: String,
}

impl Veth{
    fn create(&self) -> anyhow::Result<()>{
        let output = Command::new("ip")
            .arg("link")
            .arg("add")
            .arg(self.name.as_str())
            .arg("type")
            .arg("veth")
            .arg("peer")
            .arg("name")
            .arg(self.peer.as_str())
            .output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to create veth: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }
}

struct Namespace{
    name: String,
}

impl Namespace {
    fn new(name: String, ecmp: bool, config: &mut Config) -> anyhow::Result<Arc<Namespace>> {
        if let Some(r) = config.namespaces.get(&name){
            return Err(anyhow::anyhow!("Namespace {} already exists", r.name));
        }
        let n= Namespace{
            name: name.clone(),
        };
        let n = Arc::new(n);
        if let Err(e) = n.create(){
            return Err(anyhow::anyhow!("Failed to create network namespace: {}", e));
        }
        n.enable_routing()?;
        if ecmp {
            n.enable_ecmp()?;
        }
        config.namespaces.insert(name.clone(), Box::new(n.clone()));
        Ok(n.clone())
    }
    fn enable_ecmp(&self) -> anyhow::Result<()>{
        let output = Command::new("ip")
            .arg("netns")
            .arg("exec")
            .arg(self.name.as_str())
            .arg("sysctl")
            .arg("-w")
            .arg("net.ipv4.fib_multipath_hash_policy=1")
        .output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to enable ecmp: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    fn enable_routing(&self) -> anyhow::Result<()>{
        let output = Command::new("ip")
            .arg("netns")
            .arg("exec")
            .arg(self.name.as_str())
            .arg("sysctl")
            .arg("-w")
            .arg("net.ipv4.ip_forward=1")
        .output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to enable routing: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    fn create(&self) -> anyhow::Result<()>{
        let output = Command::new("ip")
            .arg("netns")
            .arg("add")
            .arg(self.name.as_str())
            .output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to create namespace: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }
    fn add_route(&self, route: Route) -> anyhow::Result<()>{
        {
            let mut args = vec![
                "netns",
                "exec",
                self.name.as_str(),
                "ip",
                "route",
                "add",
                route.dst.as_str(),
            ];
            for intf in &route.gateway{
                let ip = if let Some(ip) = &intf.ip{
                    let ip_vec: Vec<&str> = ip.split("/").collect();
                    ip_vec[0]
                } else {
                    return Err(anyhow::anyhow!("Interface {} does not have an IP address", intf.name));
                };
                args.push("nexthop");
                args.push("via");
                args.push(ip);
                if route.gateway.len() > 1 {
                    args.push("weight");
                    args.push("1");
                }
            }
            Command::new("ip").args(args).output()?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Error>{
    let mut config = Config::new();

    let r1 = Namespace::new("r1".to_string(), true, &mut config)?;
    let r2 = Namespace::new("r2".to_string(), true, &mut config)?;

    let link1 = Link::new(
        "link1".to_string(),
        "10.0.0.0/24".to_string(),
        &mut config,
    )?;
    let (link1_intf1, link1_intf2) = link1.attach(r1.clone(), r2.clone(), &mut config)?;

    let link2 = Link::new(
        "link2".to_string(),
        "10.0.1.0/24".to_string(),
        &mut config,
    )?;
    let (link2_intf1, link2_intf2) = link2.attach(r1.clone(), r2.clone(), &mut config)?;

    let link3 = Link::new(
        "link3".to_string(),
        "10.0.2.0/24".to_string(),
        &mut config,
    )?;
    let (link3_intf1, link3_intf2) = link3.attach(r1.clone(), r2.clone(), &mut config)?;

    let link4 = Link::new(
        "link4".to_string(),
        "10.0.3.0/24".to_string(),
        &mut config,
    )?;
    let (link4_intf1, link4_intf2) = link4.attach(r1.clone(), r2.clone(), &mut config)?;

    let link5 = Link::new(
        "link5".to_string(),
        "10.0.4.0/24".to_string(),
        &mut config,
    )?;
    let (link5_intf1, link5_intf2) = link5.attach(r1.clone(), r2.clone(), &mut config)?;

    let link6 = Link::new(
        "link6".to_string(),
        "10.0.5.0/24".to_string(),
        &mut config,
    )?;
    let (link6_intf1, link6_intf2) = link6.attach(r1.clone(), r2.clone(), &mut config)?;

    let p1 = Namespace::new("p1".to_string(), false, &mut config)?;
    let plink1 = Link::new(
        "plink1".to_string(),
        "10.1.2.0/24".to_string(),
         &mut config
    )?;
    let (p1_intf1,p1_intf2) = plink1.attach(p1.clone(), r1.clone(), &mut config)?;

    let p2 = Namespace::new("p2".to_string(), false, &mut config)?;
    let plink2 = Link::new(
        "plink2".to_string(),
        "10.1.3.0/24".to_string(),
        &mut config
    )?;
    let (p2_intf1,p2_intf2) = plink2.attach(p2.clone(), r2.clone(), &mut config)?;

    Interface::new(
        "en0".to_string(),
        Some(p1.clone()),
        Some("192.168.0.1/24".to_string()),
        Some(3000),
        &mut config,
    )?;

    Interface::new(
        "en1".to_string(),
        Some(p2.clone()),
        Some("192.168.1.1/24".to_string()),
        Some(3000),
        &mut config,
    )?;

    r1.add_route(Route {
        dst: "192.168.1.0/24".to_string(), 
        gateway: vec![
            link1_intf2.clone(),
            link2_intf2.clone(),
            link3_intf2.clone(),
            link4_intf2.clone(),
            link5_intf2.clone(),
            link6_intf2.clone(),
        ],
    })?;

    r1.add_route(Route {
        dst: "192.168.0.0/24".to_string(), 
        gateway: vec![
            p1_intf1.clone(),
        ],
    })?;

    p1.add_route(Route {
        dst: "192.168.1.0/24".to_string(), 
        gateway: vec![
            p1_intf2.clone(),
        ],
    })?;

    r2.add_route(Route {
        dst: "192.168.0.0/24".to_string(), 
        gateway: vec![
            link1_intf1.clone(),
            link2_intf1.clone(),
            link3_intf1.clone(),
            link4_intf1.clone(),
            link5_intf1.clone(),
            link6_intf1.clone(),
        ],
    })?;

    r2.add_route(Route {
        dst: "192.168.1.0/24".to_string(), 
        gateway: vec![
            p2_intf1.clone(),
        ],
    })?;

    p2.add_route(Route {
        dst: "192.168.0.0/24".to_string(), 
        gateway: vec![
            p2_intf2.clone(),
        ],
    })?;
    Ok(())
}